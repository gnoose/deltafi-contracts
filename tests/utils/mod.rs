#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

use assert_matches::*;
use deltafi_swap::{
    curve::{Multiplier, PoolState},
    instruction::{
        deposit, init_liquidity_provider, initialize, initialize_config, swap, withdraw,
        DepositData, InitializeData, SwapData, SwapDirection, WithdrawData,
    },
    math::Decimal,
    pyth,
    state::{
        ConfigInfo, Fees, LiquidityPosition, LiquidityProvider, Rewards, SwapInfo, PROGRAM_VERSION,
    },
};
use solana_program::{program_option::COption, program_pack::Pack, pubkey::Pubkey};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    system_instruction::create_account,
    transaction::Transaction,
};
use spl_token::{
    instruction::{approve, initialize_account, initialize_mint, set_authority, AuthorityType},
    native_mint::DECIMALS,
    state::{Account as Token, AccountState, Mint},
};
use std::{convert::TryInto, str::FromStr};

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

pub const SRM_MINT: &str = "SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt";

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

pub struct TestOracle {
    pub product_pubkey: Pubkey,
    pub price_pubkey: Pubkey,
    pub price: Decimal,
}

pub struct TestMint {
    pub pubkey: Pubkey,
    pub authority: Keypair,
    pub decimals: u8,
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

#[derive(Default)]
pub struct AddSwapInfoArgs {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub is_open_twap: bool,
    pub oracle_a: Pubkey,
    pub oracle_b: Pubkey,
    pub market_price: Decimal,
    pub slope: Decimal,
}

pub fn add_swap_info(
    test: &mut ProgramTest,
    swap_config: &TestSwapConfig,
    user_account_owner: &Keypair,
    admin_account_owner: &Keypair,
    args: AddSwapInfoArgs,
) -> TestSwapInfo {
    let AddSwapInfoArgs {
        token_a_mint,
        token_b_mint,
        token_a_amount,
        token_b_amount,
        is_open_twap,
        oracle_a,
        oracle_b,
        market_price,
        slope,
    } = args;

    let mut pool_state = PoolState::new(PoolState {
        market_price,
        slope,
        base_target: Decimal::zero(),
        quote_target: Decimal::zero(),
        base_reserve: Decimal::zero(),
        quote_reserve: Decimal::zero(),
        multiplier: Multiplier::One,
    })
    .unwrap();

    let pool_mint_amount = pool_state
        .buy_shares(token_a_amount, token_b_amount, 0)
        .unwrap();

    let swap_info_pubkey = Pubkey::new_unique();
    let (swap_authority_pubkey, nonce) =
        Pubkey::find_program_address(&[swap_info_pubkey.as_ref()], &deltafi_swap::id());

    let pool_mint = Pubkey::new_unique();
    test.add_packable_account(
        pool_mint,
        u32::MAX as u64,
        &Mint {
            is_initialized: true,
            decimals: DECIMALS,
            mint_authority: COption::Some(swap_authority_pubkey),
            freeze_authority: COption::None,
            supply: pool_mint_amount,
            ..Mint::default()
        },
        &spl_token::id(),
    );

    let pool_token = Pubkey::new_unique();
    test.add_packable_account(
        pool_token,
        u32::MAX as u64,
        &Token {
            mint: pool_mint,
            owner: user_account_owner.pubkey(),
            amount: pool_mint_amount,
            state: AccountState::Initialized,
            ..Token::default()
        },
        &spl_token::id(),
    );

    let token_a = Pubkey::new_unique();
    test.add_packable_account(
        token_a,
        u32::MAX as u64,
        &Token {
            mint: token_a_mint,
            owner: swap_authority_pubkey,
            amount: token_a_amount,
            state: AccountState::Initialized,
            ..Token::default()
        },
        &spl_token::id(),
    );

    let token_b = Pubkey::new_unique();
    test.add_packable_account(
        token_b,
        u32::MAX as u64,
        &Token {
            mint: token_b_mint,
            owner: swap_authority_pubkey,
            amount: token_b_amount,
            state: AccountState::Initialized,
            ..Token::default()
        },
        &spl_token::id(),
    );

    let admin_fee_a_key = Pubkey::new_unique();
    test.add_packable_account(
        admin_fee_a_key,
        u32::MAX as u64,
        &Token {
            mint: token_a_mint,
            owner: admin_account_owner.pubkey(),
            amount: 0,
            state: AccountState::Initialized,
            ..Token::default()
        },
        &spl_token::id(),
    );

    let admin_fee_b_key = Pubkey::new_unique();
    test.add_packable_account(
        admin_fee_b_key,
        u32::MAX as u64,
        &Token {
            mint: token_b_mint,
            owner: admin_account_owner.pubkey(),
            amount: 0,
            state: AccountState::Initialized,
            ..Token::default()
        },
        &spl_token::id(),
    );

    let swap_info = SwapInfo {
        is_initialized: true,
        is_paused: false,
        nonce,
        token_a,
        token_b,
        pool_mint,
        token_a_mint,
        token_b_mint,
        admin_fee_key_a: admin_fee_a_key,
        admin_fee_key_b: admin_fee_b_key,
        fees: swap_config.fees.clone(),
        rewards: swap_config.rewards.clone(),
        is_open_twap,
        pool_state,
        ..SwapInfo::default()
    };

    test.add_packable_account(
        swap_info_pubkey,
        u32::MAX as u64,
        &swap_info,
        &deltafi_swap::id(),
    );

    TestSwapInfo {
        pubkey: swap_info_pubkey,
        authority: swap_authority_pubkey,
        nonce,
        token_a,
        token_b,
        pool_token,
        pool_mint,
        token_a_mint,
        token_b_mint,
        admin_fee_a_key,
        admin_fee_b_key,
        is_open_twap,
        fees: swap_config.fees.clone(),
        rewards: swap_config.rewards.clone(),
        oracle_a,
        oracle_b,
    }
}

pub fn add_liquidity_provider(
    test: &mut ProgramTest,
    user_account_owner: &Keypair,
) -> TestLiquidityProvider {
    let liquidity_provider_pubkey = Pubkey::new_unique();
    test.add_packable_account(
        liquidity_provider_pubkey,
        u32::MAX as u64,
        &LiquidityProvider {
            is_initialized: true,
            owner: user_account_owner.pubkey(),
            positions: vec![],
        },
        &deltafi_swap::id(),
    );

    TestLiquidityProvider {
        pubkey: liquidity_provider_pubkey,
        owner: user_account_owner.pubkey(),
        positions: vec![],
    }
}

pub fn add_position(
    test: &mut ProgramTest,
    swap_info: &TestSwapInfo,
    user_account_owner: &Keypair,
    liquidity_amount: u64,
) -> TestLiquidityProvider {
    let liquidity_provider_pubkey = Pubkey::new_unique();
    let mut liquidity_provider = LiquidityProvider {
        is_initialized: true,
        owner: user_account_owner.pubkey(),
        positions: vec![],
    };
    liquidity_provider
        .find_or_add_position(swap_info.pubkey, 0)
        .unwrap()
        .deposit(liquidity_amount)
        .unwrap();

    test.add_packable_account(
        liquidity_provider_pubkey,
        u32::MAX as u64,
        &liquidity_provider,
        &deltafi_swap::id(),
    );

    TestLiquidityProvider {
        pubkey: liquidity_provider_pubkey,
        owner: user_account_owner.pubkey(),
        positions: liquidity_provider.positions,
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

pub struct TestSwapInfo {
    pub pubkey: Pubkey,
    pub authority: Pubkey,
    pub nonce: u8,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub pool_token: Pubkey,
    pub pool_mint: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub admin_fee_a_key: Pubkey,
    pub admin_fee_b_key: Pubkey,
    pub is_open_twap: bool,
    pub fees: Fees,
    pub rewards: Rewards,
    pub oracle_a: Pubkey,
    pub oracle_b: Pubkey,
}

pub struct SwapInitArgs {
    pub mid_price: u128,
    pub slope: u64,
    pub is_open_twap: bool,
}

impl TestSwapInfo {
    pub async fn init(
        banks_client: &mut BanksClient,
        swap_config: &TestSwapConfig,
        cracle_a: &TestOracle,
        oracle_b: &TestOracle,
        token_a_mint: Pubkey,
        token_b_mint: Pubkey,
        token_a: Pubkey,
        token_b: Pubkey,
        admin_fee_a_key: Pubkey,
        admin_fee_b_key: Pubkey,
        user_account_owner: &Keypair,
        payer: &Keypair,
        args: &SwapInitArgs,
    ) -> Self {
        let swap_info = Keypair::new();
        let swap_info_pubkey = swap_info.pubkey();

        let (swap_authority_pubkey, nonce) = Pubkey::find_program_address(
            &[&swap_info_pubkey.to_bytes()[..32]],
            &deltafi_swap::id(),
        );

        let pool_mint_keypair = Keypair::new();
        let user_pool_token_keypair = Keypair::new();

        let rent = banks_client.get_rent().await.unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[
                create_account(
                    &payer.pubkey(),
                    &pool_mint_keypair.pubkey(),
                    rent.minimum_balance(Mint::LEN),
                    Mint::LEN as u64,
                    &spl_token::id(),
                ),
                initialize_mint(
                    &spl_token::id(),
                    &pool_mint_keypair.pubkey(),
                    &swap_authority_pubkey,
                    None,
                    DECIMALS,
                )
                .unwrap(),
                create_account(
                    &payer.pubkey(),
                    &user_pool_token_keypair.pubkey(),
                    rent.minimum_balance(Token::LEN),
                    Token::LEN as u64,
                    &spl_token::id(),
                ),
                initialize_account(
                    &spl_token::id(),
                    &user_pool_token_keypair.pubkey(),
                    &pool_mint_keypair.pubkey(),
                    &user_account_owner.pubkey(),
                )
                .unwrap(),
                set_authority(
                    &spl_token::id(),
                    &token_a,
                    Some(&swap_authority_pubkey),
                    AuthorityType::AccountOwner,
                    &user_account_owner.pubkey(),
                    &[],
                )
                .unwrap(),
                set_authority(
                    &spl_token::id(),
                    &token_b,
                    Some(&swap_authority_pubkey),
                    AuthorityType::AccountOwner,
                    &user_account_owner.pubkey(),
                    &[],
                )
                .unwrap(),
                create_account(
                    &payer.pubkey(),
                    &swap_info_pubkey,
                    rent.minimum_balance(SwapInfo::LEN),
                    SwapInfo::LEN as u64,
                    &deltafi_swap::id(),
                ),
                initialize(
                    deltafi_swap::id(),
                    swap_config.pubkey,
                    swap_info_pubkey,
                    swap_authority_pubkey,
                    admin_fee_a_key,
                    admin_fee_b_key,
                    token_a,
                    token_b,
                    pool_mint_keypair.pubkey(),
                    user_pool_token_keypair.pubkey(),
                    cracle_a.price_pubkey,
                    oracle_b.product_pubkey,
                    InitializeData {
                        nonce,
                        mid_price: args.mid_price,
                        slope: args.slope,
                        is_open_twap: args.is_open_twap,
                    },
                )
                .unwrap(),
            ],
            Some(&payer.pubkey()),
        );

        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(
            &vec![
                payer,
                user_account_owner,
                &swap_info,
                &pool_mint_keypair,
                &user_pool_token_keypair,
            ],
            recent_blockhash,
        );

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

        Self {
            pubkey: swap_info_pubkey,
            authority: swap_authority_pubkey,
            nonce,
            token_a,
            token_b,
            pool_token: user_pool_token_keypair.pubkey(),
            pool_mint: pool_mint_keypair.pubkey(),
            admin_fee_a_key,
            admin_fee_b_key,
            token_a_mint,
            token_b_mint,
            is_open_twap: args.is_open_twap,
            fees: swap_config.fees.clone(),
            rewards: swap_config.rewards.clone(),
            oracle_a: cracle_a.price_pubkey,
            oracle_b: oracle_b.price_pubkey,
        }
    }

    pub async fn swap(
        &self,
        banks_client: &mut BanksClient,
        config_info: &TestSwapConfig,
        user_account_owner: &Keypair,
        source_pubkey: Pubkey,
        destination_pubkey: Pubkey,
        reward_token_pubkey: Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        swap_direction: SwapDirection,
        payer: &Keypair,
    ) {
        let user_transfer_authority = Keypair::new();
        let mut transaction = Transaction::new_with_payer(
            &[
                approve(
                    &spl_token::id(),
                    &source_pubkey,
                    &user_transfer_authority.pubkey(),
                    &user_account_owner.pubkey(),
                    &[],
                    amount_in,
                )
                .unwrap(),
                swap(
                    deltafi_swap::id(),
                    config_info.pubkey,
                    self.pubkey,
                    config_info.market_authority,
                    self.authority,
                    user_transfer_authority.pubkey(),
                    source_pubkey,
                    self.token_a,
                    self.token_b,
                    destination_pubkey,
                    reward_token_pubkey,
                    config_info.deltafi_mint,
                    self.admin_fee_b_key,
                    self.oracle_a,
                    self.oracle_b,
                    SwapData {
                        amount_in,
                        minimum_amount_out,
                        swap_direction,
                    },
                )
                .unwrap(),
            ],
            Some(&payer.pubkey()),
        );

        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(
            &[payer, user_account_owner, &user_transfer_authority],
            recent_blockhash,
        );

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
    }

    pub async fn deposit(
        &self,
        banks_client: &mut BanksClient,
        liquidity_provider: &TestLiquidityProvider,
        user_account_owner: &Keypair,
        deposit_token_a_pubkey: Pubkey,
        deposit_token_b_pubkey: Pubkey,
        pool_token_pubkey: Pubkey,
        token_a_amount: u64,
        token_b_amount: u64,
        min_mint_amount: u64,
        payer: &Keypair,
    ) {
        let user_transfer_authority = Keypair::new();
        let mut transaction = Transaction::new_with_payer(
            &[
                approve(
                    &spl_token::id(),
                    &deposit_token_a_pubkey,
                    &user_transfer_authority.pubkey(),
                    &user_account_owner.pubkey(),
                    &[],
                    token_a_amount,
                )
                .unwrap(),
                approve(
                    &spl_token::id(),
                    &deposit_token_b_pubkey,
                    &user_transfer_authority.pubkey(),
                    &user_account_owner.pubkey(),
                    &[],
                    token_b_amount,
                )
                .unwrap(),
                deposit(
                    deltafi_swap::id(),
                    self.pubkey,
                    self.authority,
                    user_transfer_authority.pubkey(),
                    deposit_token_a_pubkey,
                    deposit_token_b_pubkey,
                    self.token_a,
                    self.token_b,
                    self.pool_mint,
                    pool_token_pubkey,
                    liquidity_provider.pubkey,
                    liquidity_provider.owner,
                    self.oracle_a,
                    self.oracle_b,
                    DepositData {
                        token_a_amount,
                        token_b_amount,
                        min_mint_amount,
                    },
                )
                .unwrap(),
            ],
            Some(&payer.pubkey()),
        );

        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(
            &[payer, user_account_owner, &user_transfer_authority],
            recent_blockhash,
        );

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
    }

    pub async fn withdraw(
        &self,
        banks_client: &mut BanksClient,
        liquidity_provider: &TestLiquidityProvider,
        user_account_owner: &Keypair,
        token_a_pubkey: Pubkey,
        token_b_pubkey: Pubkey,
        pool_token_pubkey: Pubkey,
        pool_token_amount: u64,
        minimum_token_a_amount: u64,
        minimum_token_b_amount: u64,
        payer: &Keypair,
    ) {
        let user_transfer_authority = Keypair::new();
        let mut transaction = Transaction::new_with_payer(
            &[
                approve(
                    &spl_token::id(),
                    &pool_token_pubkey,
                    &user_transfer_authority.pubkey(),
                    &user_account_owner.pubkey(),
                    &[],
                    pool_token_amount,
                )
                .unwrap(),
                withdraw(
                    deltafi_swap::id(),
                    self.pubkey,
                    self.authority,
                    user_transfer_authority.pubkey(),
                    self.pool_mint,
                    pool_token_pubkey,
                    self.token_a,
                    self.token_b,
                    token_a_pubkey,
                    token_b_pubkey,
                    self.admin_fee_a_key,
                    self.admin_fee_b_key,
                    liquidity_provider.pubkey,
                    liquidity_provider.owner,
                    self.oracle_a,
                    self.oracle_b,
                    WithdrawData {
                        pool_token_amount,
                        minimum_token_a_amount,
                        minimum_token_b_amount,
                    },
                )
                .unwrap(),
            ],
            Some(&payer.pubkey()),
        );

        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(
            &[payer, user_account_owner, &user_transfer_authority],
            recent_blockhash,
        );

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
    }

    pub async fn get_state(&self, banks_client: &mut BanksClient) -> SwapInfo {
        let swap_account: Account = banks_client
            .get_account(self.pubkey)
            .await
            .unwrap()
            .unwrap();
        SwapInfo::unpack(&swap_account.data[..]).unwrap()
    }

    pub async fn validate_state(&self, banks_client: &mut BanksClient) {
        let swap_info = self.get_state(banks_client).await;
        assert!(swap_info.is_initialized);
        assert_eq!(swap_info.token_a, self.token_a);
        assert_eq!(swap_info.token_b, self.token_b);
        assert_eq!(swap_info.admin_fee_key_a, self.admin_fee_a_key);
        assert_eq!(swap_info.admin_fee_key_b, self.admin_fee_b_key);
        assert_eq!(swap_info.token_a_mint, self.token_a_mint);
        assert_eq!(swap_info.token_b_mint, self.token_b_mint);
        assert_eq!(swap_info.is_open_twap, self.is_open_twap);
        assert_eq!(swap_info.fees, self.fees);
        assert_eq!(swap_info.rewards, self.rewards);
    }
}

pub struct TestLiquidityProvider {
    pub pubkey: Pubkey,
    pub owner: Pubkey,
    pub positions: Vec<LiquidityPosition>,
}

impl TestLiquidityProvider {
    pub async fn init(
        banks_client: &mut BanksClient,
        user_account_owner: &Keypair,
        payer: &Keypair,
    ) -> Self {
        let liquidity_provider = Keypair::new();
        let liquidity_provider_pubkey = liquidity_provider.pubkey();

        let rent = banks_client.get_rent().await.unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[
                create_account(
                    &payer.pubkey(),
                    &liquidity_provider_pubkey,
                    rent.minimum_balance(LiquidityProvider::LEN),
                    LiquidityProvider::LEN as u64,
                    &deltafi_swap::id(),
                ),
                init_liquidity_provider(
                    deltafi_swap::id(),
                    liquidity_provider_pubkey,
                    user_account_owner.pubkey(),
                )
                .unwrap(),
            ],
            Some(&payer.pubkey()),
        );

        let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
        transaction.sign(
            &vec![payer, &liquidity_provider, user_account_owner],
            recent_blockhash,
        );

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

        Self {
            pubkey: liquidity_provider_pubkey,
            owner: user_account_owner.pubkey(),
            positions: vec![],
        }
    }

    pub async fn get_state(&self, banks_client: &mut BanksClient) -> LiquidityProvider {
        let liquidity_provider: Account = banks_client
            .get_account(self.pubkey)
            .await
            .unwrap()
            .unwrap();
        LiquidityProvider::unpack(&liquidity_provider.data[..]).unwrap()
    }

    pub async fn validate_state(&self, banks_client: &mut BanksClient) {
        let liquidity_provider = self.get_state(banks_client).await;
        assert!(liquidity_provider.is_initialized);
        assert_eq!(liquidity_provider.owner, self.owner);
    }
}

pub async fn create_and_mint_to_token_account(
    banks_client: &mut BanksClient,
    mint_pubkey: Pubkey,
    mint_authority: Option<&Keypair>,
    payer: &Keypair,
    authority: Pubkey,
    amount: u64,
) -> Pubkey {
    if let Some(mint_authority) = mint_authority {
        let account_pubkey =
            create_token_account(banks_client, mint_pubkey, &payer, Some(authority), None).await;

        mint_to(
            banks_client,
            mint_pubkey,
            &payer,
            account_pubkey,
            mint_authority,
            amount,
        )
        .await;

        account_pubkey
    } else {
        create_token_account(
            banks_client,
            mint_pubkey,
            &payer,
            Some(authority),
            Some(amount),
        )
        .await
    }
}

pub async fn create_token_account(
    banks_client: &mut BanksClient,
    mint_pubkey: Pubkey,
    payer: &Keypair,
    authority: Option<Pubkey>,
    native_amount: Option<u64>,
) -> Pubkey {
    let token_keypair = Keypair::new();
    let token_pubkey = token_keypair.pubkey();
    let authority_pubkey = authority.unwrap_or_else(|| payer.pubkey());

    let rent = banks_client.get_rent().await.unwrap();
    let lamports = rent.minimum_balance(Token::LEN) + native_amount.unwrap_or_default();
    let mut transaction = Transaction::new_with_payer(
        &[
            create_account(
                &payer.pubkey(),
                &token_pubkey,
                lamports,
                Token::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_account(
                &spl_token::id(),
                &token_pubkey,
                &mint_pubkey,
                &authority_pubkey,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
    transaction.sign(&[&payer, &token_keypair], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    token_pubkey
}

pub async fn mint_to(
    banks_client: &mut BanksClient,
    mint_pubkey: Pubkey,
    payer: &Keypair,
    account_pubkey: Pubkey,
    authority: &Keypair,
    amount: u64,
) {
    let mut transaction = Transaction::new_with_payer(
        &[spl_token::instruction::mint_to(
            &spl_token::id(),
            &mint_pubkey,
            &account_pubkey,
            &authority.pubkey(),
            &[],
            amount,
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );

    let recent_blockhash = banks_client.get_recent_blockhash().await.unwrap();
    transaction.sign(&[payer, authority], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}

pub async fn get_token_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
    let token: Account = banks_client.get_account(pubkey).await.unwrap().unwrap();

    spl_token::state::Account::unpack(&token.data[..])
        .unwrap()
        .amount
}

pub fn add_oracle(
    test: &mut ProgramTest,
    product_pubkey: Pubkey,
    price_pubkey: Pubkey,
    price: Decimal,
) -> TestOracle {
    let oracle_program_id = read_keypair_file("tests/fixtures/pyth_program_id.json").unwrap();

    // Add Pyth product account
    test.add_account_with_file_data(
        product_pubkey,
        u32::MAX as u64,
        oracle_program_id.pubkey(),
        &format!("{}.bin", product_pubkey.to_string()),
    );

    // Add Pyth price account after setting the price
    let filename = &format!("{}.bin", price_pubkey.to_string());
    let mut pyth_price_data = read_file(find_file(filename).unwrap_or_else(|| {
        panic!("Unable to locate {}", filename);
    }));

    let mut pyth_price = pyth::load_mut::<pyth::Price>(pyth_price_data.as_mut_slice()).unwrap();

    let decimals = 10u64
        .checked_pow(pyth_price.expo.checked_abs().unwrap().try_into().unwrap())
        .unwrap();

    pyth_price.valid_slot = 0;
    pyth_price.agg.price = price
        .try_round_u64()
        .unwrap()
        .checked_mul(decimals)
        .unwrap()
        .try_into()
        .unwrap();

    test.add_account(
        price_pubkey,
        Account {
            lamports: u32::MAX as u64,
            data: pyth_price_data,
            owner: oracle_program_id.pubkey(),
            executable: false,
            rent_epoch: 0,
        },
    );

    TestOracle {
        product_pubkey,
        price_pubkey,
        price,
    }
}

pub fn add_sol_oracle(test: &mut ProgramTest) -> TestOracle {
    add_oracle(
        test,
        Pubkey::from_str(SOL_PYTH_PRODUCT).unwrap(),
        Pubkey::from_str(SOL_PYTH_PRICE).unwrap(),
        // Set SOL price to $20
        Decimal::from(150u64),
    )
}

pub fn add_srm_oracle(test: &mut ProgramTest) -> TestOracle {
    add_oracle(
        test,
        // Mock with SRM since Pyth doesn't have USDC yet
        Pubkey::from_str(SRM_PYTH_PRODUCT).unwrap(),
        Pubkey::from_str(SRM_PYTH_PRICE).unwrap(),
        // Set USDC price to $1
        Decimal::from(7u64),
    )
}

pub fn add_srm_mint(test: &mut ProgramTest) -> TestMint {
    let authority = Keypair::new();
    let pubkey = Pubkey::from_str(SRM_MINT).unwrap();
    let decimals = DECIMALS;
    test.add_packable_account(
        pubkey,
        u32::MAX as u64,
        &Mint {
            is_initialized: true,
            mint_authority: COption::Some(authority.pubkey()),
            decimals,
            ..Mint::default()
        },
        &spl_token::id(),
    );

    TestMint {
        pubkey,
        authority,
        decimals,
    }
}
