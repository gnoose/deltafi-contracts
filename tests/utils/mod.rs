#![allow(dead_code)]

use assert_matches::*;
use deltafi_swap::{
    instruction::initialize_config,
    state::{ConfigInfo, Fees, Rewards, PROGRAM_VERSION},
};
use solana_program::{
    program_option::COption, program_pack::Pack, pubkey::Pubkey, system_instruction::create_account,
};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    transaction::Transaction,
};
use spl_token::{instruction::initialize_mint, native_mint::DECIMALS, state::Mint};

pub const LAMPORTS_TO_SOL: u64 = 1_000_000_000;
pub const FRACTIONAL_TO_USDC: u64 = 1_000_000;

pub const ZERO_TS: i64 = 0;

pub const TEST_FEES: Fees = Fees {
    admin_trade_fee_numerator: 2,
    admin_trade_fee_denominator: 5,
    admin_withdraw_fee_numerator: 2,
    admin_withdraw_fee_denominator: 5,
    trade_fee_numerator: 5,
    trade_fee_denominator: 1_000,
    withdraw_fee_numerator: 2,
    withdraw_fee_denominator: 100,
};

pub const TEST_REWARDS: Rewards = Rewards {
    trade_reward_numerator: 1,
    trade_reward_denominator: 1_000,
    trade_reward_cap: 10_000_000_000,
    liquidity_reward_numerator: 1,
    liquidity_reward_denominator: 1_000,
};

pub const SOL_PYTH_PRODUCT: &str = "3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E";
pub const SOL_PYTH_PRICE: &str = "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix";

pub const SRM_PYTH_PRODUCT: &str = "6MEwdxe4g1NeAF9u6KDG14anJpFsVEa2cvr5H6iriFZ8";
pub const SRM_PYTH_PRICE: &str = "992moaMQKs32GKZ9dxi8keyM2bUmbrwBZpK4p2K6X5Vs";

trait AddPacked {
    fn add_packable_account<T: Pack>(
        &mut self,
        pubkey: Pubkey,
        amount: u64,
        data: &T,
        owner: &Pubkey,
    );
}

impl AddPacked for ProgramTest {
    fn add_packable_account<T: Pack>(
        &mut self,
        pubkey: Pubkey,
        amount: u64,
        data: &T,
        owner: &Pubkey,
    ) {
        let mut account = Account::new(amount, T::get_packed_len(), owner);
        data.pack_into_slice(&mut account.data);
        self.add_account(pubkey, account);
    }
}

pub fn add_swap_config(test: &mut ProgramTest) -> TestSwapConfig {
    let swap_config_pubkey = Pubkey::new_unique();
    let (market_authority, bump_seed) =
        Pubkey::find_program_address(&[swap_config_pubkey.as_ref()], &deltafi_swap::id());

    let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();

    let deltafi_mint = Pubkey::new_unique();
    test.add_packable_account(
        deltafi_mint,
        u32::MAX as u64,
        &Mint {
            is_initialized: true,
            decimals: DECIMALS,
            mint_authority: COption::Some(market_authority),
            freeze_authority: COption::Some(admin.pubkey()),
            supply: 0,
            ..Mint::default()
        },
        &spl_token::id(),
    );

    test.add_packable_account(
        swap_config_pubkey,
        u32::MAX as u64,
        &ConfigInfo {
            version: PROGRAM_VERSION,
            bump_seed,
            admin_key: admin.pubkey(),
            deltafi_mint,
            fees: TEST_FEES,
            rewards: TEST_REWARDS,
        },
        &deltafi_swap::id(),
    );

    TestSwapConfig {
        pubkey: swap_config_pubkey,
        admin,
        market_authority,
        deltafi_mint,
        fees: TEST_FEES,
        rewards: TEST_REWARDS,
    }
}

pub struct TestSwapConfig {
    pub pubkey: Pubkey,
    pub admin: Keypair,
    pub market_authority: Pubkey,
    pub deltafi_mint: Pubkey,
    pub fees: Fees,
    pub rewards: Rewards,
}

impl TestSwapConfig {
    pub async fn init(banks_client: &mut BanksClient, payer: &Keypair) -> Self {
        let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();
        let admin_pubkey = admin.pubkey();
        let swap_config_keypair = Keypair::new();
        let swap_config_pubkey = swap_config_keypair.pubkey();
        let (market_authority_pubkey, _bump_seed) = Pubkey::find_program_address(
            &[&swap_config_pubkey.to_bytes()[..32]],
            &deltafi_swap::id(),
        );
        let deltafi_mint = Keypair::new();

        let rent = banks_client.get_rent().await.unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[
                create_account(
                    &payer.pubkey(),
                    &deltafi_mint.pubkey(),
                    rent.minimum_balance(Mint::LEN),
                    Mint::LEN as u64,
                    &spl_token::id(),
                ),
                initialize_mint(
                    &spl_token::id(),
                    &deltafi_mint.pubkey(),
                    &market_authority_pubkey,
                    Some(&admin_pubkey),
                    DECIMALS,
                )
                .unwrap(),
                create_account(
                    &payer.pubkey(),
                    &swap_config_pubkey,
                    rent.minimum_balance(ConfigInfo::LEN),
                    ConfigInfo::LEN as u64,
                    &deltafi_swap::id(),
                ),
                initialize_config(
                    deltafi_swap::id(),
                    swap_config_pubkey,
                    market_authority_pubkey,
                    deltafi_mint.pubkey(),
                    admin_pubkey,
                    TEST_FEES,
                    TEST_REWARDS,
                )
                .unwrap(),
            ],
            Some(&payer.pubkey()),
        );

        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(
            &[payer, &admin, &swap_config_keypair, &deltafi_mint],
            recent_blockhash,
        );

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

        Self {
            pubkey: swap_config_pubkey,
            admin,
            market_authority: market_authority_pubkey,
            deltafi_mint: deltafi_mint.pubkey(),
            fees: TEST_FEES,
            rewards: TEST_REWARDS,
        }
    }

    pub async fn get_state(&self, banks_client: &mut BanksClient) -> ConfigInfo {
        let swap_config_account: Account = banks_client
            .get_account(self.pubkey)
            .await
            .unwrap()
            .unwrap();
        ConfigInfo::unpack(&swap_config_account.data[..]).unwrap()
    }

    pub async fn validate_state(&self, banks_client: &mut BanksClient) {
        let swap_config = self.get_state(banks_client).await;
        assert_eq!(swap_config.version, PROGRAM_VERSION);
        assert_eq!(swap_config.admin_key, self.admin.pubkey());
        assert_eq!(swap_config.deltafi_mint, self.deltafi_mint);
        assert_eq!(swap_config.fees, self.fees);
        assert_eq!(swap_config.rewards, self.rewards);
    }
}
