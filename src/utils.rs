//! Utility methods

use solana_program::{program_pack::Pack, pubkey::Pubkey};
use spl_token::state::Account;

use crate::error::SwapError;

/// swap directions - sell base
pub const SWAP_DIRECTION_SELL_BASE: u64 = 0;

/// swap directions - sell quote
pub const SWAP_DIRECTION_SELL_QUOTE: u64 = 1;

/// Default token decimals
pub const DEFAULT_TOKEN_DECIMALS: u8 = 6;

/// Default base point with default token decimals
pub const DEFAULT_BASE_POINT: u64 = 1000000;

/// Open Twap Flag
pub const TWAP_OPENED: u64 = 1;

/// Close Twap Falg
pub const TWAP_CLOSED: u64 = 0;

/// Calculates the authority id by generating a program address.
pub fn authority_id(program_id: &Pubkey, my_info: &Pubkey, nonce: u8) -> Result<Pubkey, SwapError> {
    Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
        .or(Err(SwapError::InvalidProgramAddress))
}

/// Unpacks a spl_token `Account`.
pub fn unpack_token_account(data: &[u8]) -> Result<Account, SwapError> {
    Account::unpack(data).map_err(|_| SwapError::ExpectedAccount)
}

#[cfg(test)]
pub mod test_utils {
    use std::time::{SystemTime, UNIX_EPOCH};

    use solana_program::{
        account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult,
        instruction::Instruction, msg, program_error::ProgramError, program_pack::Pack,
        program_stubs, pubkey::Pubkey, rent::Rent, sysvar::id,
    };
    use solana_sdk::account::{create_account, create_is_signer_account_infos, Account};
    use spl_token::{
        instruction::{approve, initialize_account, initialize_mint, mint_to},
        state::{Account as SplAccount, Mint as SplMint},
    };

    use super::*;
    use crate::{
        bn::FixedU256, 
        curve::ZERO_TS,
        fees::Fees,
        instruction::*,
        processor::Processor,
        rewards::Rewards,
        state::{FarmBaseInfo, FarmInfo, FarmingUserInfo, SwapInfo},
    };

    /// Test program id for the swap program.
    pub const SWAP_PROGRAM_ID: Pubkey = Pubkey::new_from_array([2u8; 32]);
    /// Test program id for the token program.
    pub const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([1u8; 32]);

    /// Fees for testing
    pub const DEFAULT_TEST_FEES: Fees = Fees {
        admin_trade_fee_numerator: 1,
        admin_trade_fee_denominator: 2,
        admin_withdraw_fee_numerator: 1,
        admin_withdraw_fee_denominator: 2,
        trade_fee_numerator: 6,
        trade_fee_denominator: 100,
        withdraw_fee_numerator: 6,
        withdraw_fee_denominator: 100,
    };

    /// Rewards for testing
    pub const DEFAULT_TEST_REWARDS: Rewards = Rewards {
        trade_reward_numerator: 1,
        trade_reward_denominator: 2,
        trade_reward_cap: 100,
    };

    /// Slope Value for testing
    pub fn default_k() -> FixedU256 {
        FixedU256::one()
            .checked_mul_floor(FixedU256::new(5.into()))
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap()
    }

    /// Mid Price for testing
    pub fn default_i() -> FixedU256 {
        FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap()
    }

    pub fn clock_account(ts: i64) -> Account {
        let clock = Clock {
            unix_timestamp: ts,
            ..Default::default()
        };
        Account::new_data(1, &clock, &id()).unwrap()
    }

    pub fn pubkey_rand() -> Pubkey {
        Pubkey::new_unique()
    }

    pub struct SwapAccountInfo {
        pub nonce: u8,
        pub authority_key: Pubkey,
        pub initial_amp_factor: u64,
        pub target_amp_factor: u64,
        pub swap_key: Pubkey,
        pub swap_account: Account,
        pub pool_mint_key: Pubkey,
        pub pool_mint_account: Account,
        pub pool_token_key: Pubkey,
        pub pool_token_account: Account,
        pub token_a_key: Pubkey,
        pub token_a_account: Account,
        pub token_a_mint_key: Pubkey,
        pub token_a_mint_account: Account,
        pub token_b_key: Pubkey,
        pub token_b_account: Account,
        pub token_b_mint_key: Pubkey,
        pub token_b_mint_account: Account,
        pub deltafi_token_key: Pubkey,
        pub deltafi_token_account: Account,
        pub deltafi_mint_key: Pubkey,
        pub deltafi_mint_account: Account,
        pub admin_key: Pubkey,
        pub admin_account: Account,
        pub admin_fee_a_key: Pubkey,
        pub admin_fee_a_account: Account,
        pub admin_fee_b_key: Pubkey,
        pub admin_fee_b_account: Account,
        pub fees: Fees,
        pub rewards: Rewards,
        pub k: FixedU256,
        pub i: FixedU256,
        pub base_target: FixedU256,
        pub quote_target: FixedU256,
        pub base_reserve: FixedU256,
        pub quote_reserve: FixedU256,
        pub is_open_twap: u64,
        pub block_timestamp_last: i64,
        pub base_price_cumulative_last: FixedU256,
    }

    impl SwapAccountInfo {
        pub fn new(
            user_key: &Pubkey,
            amp_factor: u64,
            token_a_amount: u64,
            token_b_amount: u64,
            fees: Fees,
            rewards: Rewards,
            k: FixedU256,
            i: FixedU256,
            is_open_twap: u64,
        ) -> Self {
            let swap_key = pubkey_rand();
            let swap_account = Account::new(0, SwapInfo::get_packed_len(), &SWAP_PROGRAM_ID);
            let (authority_key, nonce) =
                Pubkey::find_program_address(&[&swap_key.to_bytes()[..]], &SWAP_PROGRAM_ID);

            let (pool_mint_key, mut pool_mint_account) = create_mint(
                &TOKEN_PROGRAM_ID,
                &authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let (pool_token_key, pool_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &pool_mint_key,
                &mut pool_mint_account,
                &authority_key,
                &user_key,
                0,
            );
            let (deltafi_mint_key, mut deltafi_mint_account) = create_mint(
                &TOKEN_PROGRAM_ID,
                &authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let (deltafi_token_key, deltafi_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &deltafi_mint_key,
                &mut deltafi_mint_account,
                &authority_key,
                &user_key,
                0,
            );
            let (token_a_mint_key, mut token_a_mint_account) =
                create_mint(&TOKEN_PROGRAM_ID, &user_key, DEFAULT_TOKEN_DECIMALS, None);
            let (token_a_key, token_a_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &token_a_mint_key,
                &mut token_a_mint_account,
                &user_key,
                &authority_key,
                token_a_amount,
            );
            let (admin_fee_a_key, admin_fee_a_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &token_a_mint_key,
                &mut token_a_mint_account,
                &user_key,
                &authority_key,
                0,
            );
            let (token_b_mint_key, mut token_b_mint_account) =
                create_mint(&TOKEN_PROGRAM_ID, &user_key, DEFAULT_TOKEN_DECIMALS, None);
            let (token_b_key, token_b_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &token_b_mint_key,
                &mut token_b_mint_account,
                &user_key,
                &authority_key,
                token_b_amount,
            );
            let (admin_fee_b_key, admin_fee_b_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &token_b_mint_key,
                &mut token_b_mint_account,
                &user_key,
                &authority_key,
                0,
            );

            let admin_account = Account::default();
            let base_target = FixedU256::zero();
            let quote_target = FixedU256::zero();
            let base_reserve = FixedU256::zero();
            let quote_reserve = FixedU256::zero();
            let block_timestamp_last = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let base_price_cumulative_last = FixedU256::zero();

            SwapAccountInfo {
                nonce,
                authority_key,
                initial_amp_factor: amp_factor,
                target_amp_factor: amp_factor,
                swap_key,
                swap_account,
                pool_mint_key,
                pool_mint_account,
                pool_token_key,
                pool_token_account,
                token_a_mint_key,
                token_a_mint_account,
                token_a_key,
                token_a_account,
                token_b_mint_key,
                token_b_mint_account,
                token_b_key,
                token_b_account,
                deltafi_mint_key,
                deltafi_mint_account,
                deltafi_token_key,
                deltafi_token_account,
                admin_key: admin_account.owner,
                admin_account,
                admin_fee_a_key,
                admin_fee_a_account,
                admin_fee_b_key,
                admin_fee_b_account,
                fees,
                rewards,
                k,
                i,
                base_target,
                quote_target,
                base_reserve,
                quote_reserve,
                is_open_twap,
                block_timestamp_last,
                base_price_cumulative_last,
            }
        }

        pub fn initialize_swap(&mut self) -> ProgramResult {
            do_process_instruction(
                initialize(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                    &self.admin_fee_a_key,
                    &self.admin_fee_b_key,
                    &self.token_a_mint_key,
                    &self.token_a_key,
                    &self.token_b_mint_key,
                    &self.token_b_key,
                    &self.pool_mint_key,
                    &self.pool_token_key,
                    &self.deltafi_mint_key,
                    &self.deltafi_token_key,
                    self.nonce,
                    self.initial_amp_factor,
                    self.fees,
                    self.rewards,
                    self.k.inner_u64()?,
                    self.i.inner_u64()?,
                    self.is_open_twap,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut self.admin_fee_a_account,
                    &mut self.admin_fee_b_account,
                    &mut self.token_a_mint_account,
                    &mut self.token_a_account,
                    &mut self.token_b_mint_account,
                    &mut self.token_b_account,
                    &mut self.pool_mint_account,
                    &mut self.pool_token_account,
                    &mut self.deltafi_mint_account,
                    &mut self.deltafi_token_account,
                    &mut Account::default(),
                ],
            )
        }

        pub fn setup_token_accounts(
            &mut self,
            mint_owner: &Pubkey,
            account_owner: &Pubkey,
            a_amount: u64,
            b_amount: u64,
            pool_amount: u64,
        ) -> (Pubkey, Account, Pubkey, Account, Pubkey, Account) {
            let (token_a_key, token_a_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &self.token_a_mint_key,
                &mut self.token_a_mint_account,
                &mint_owner,
                &account_owner,
                a_amount,
            );
            let (token_b_key, token_b_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &self.token_b_mint_key,
                &mut self.token_b_mint_account,
                &mint_owner,
                &account_owner,
                b_amount,
            );
            let (pool_key, pool_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &self.pool_mint_key,
                &mut self.pool_mint_account,
                &self.authority_key,
                &account_owner,
                pool_amount,
            );
            (
                token_a_key,
                token_a_account,
                token_b_key,
                token_b_account,
                pool_key,
                pool_account,
            )
        }

        fn get_admin_fee_key(&self, account_key: &Pubkey) -> Pubkey {
            if *account_key == self.token_a_key {
                return self.admin_fee_a_key;
            } else if *account_key == self.token_b_key {
                return self.admin_fee_b_key;
            }
            panic!("Could not find matching admin fee account");
        }

        fn get_admin_fee_account(&self, account_key: &Pubkey) -> &Account {
            if *account_key == self.admin_fee_a_key {
                return &self.admin_fee_a_account;
            } else if *account_key == self.admin_fee_b_key {
                return &self.admin_fee_b_account;
            }
            panic!("Could not find matching admin fee account");
        }

        fn set_admin_fee_account_(&mut self, account_key: &Pubkey, account: Account) {
            if *account_key == self.admin_fee_a_key {
                self.admin_fee_a_account = account;
                return;
            } else if *account_key == self.admin_fee_b_key {
                self.admin_fee_b_account = account;
                return;
            }
            panic!("Could not find matching admin fee account");
        }

        fn get_token_account(&self, account_key: &Pubkey) -> &Account {
            if *account_key == self.token_a_key {
                return &self.token_a_account;
            } else if *account_key == self.token_b_key {
                return &self.token_b_account;
            }
            panic!("Could not find matching swap token account");
        }

        fn set_token_account(&mut self, account_key: &Pubkey, account: Account) {
            if *account_key == self.token_a_key {
                self.token_a_account = account;
                return;
            } else if *account_key == self.token_b_key {
                self.token_b_account = account;
                return;
            }
            panic!("Could not find matching swap token account");
        }

        #[allow(clippy::too_many_arguments)]
        pub fn swap(
            &mut self,
            user_key: &Pubkey,
            user_source_key: &Pubkey,
            mut user_source_account: &mut Account,
            swap_source_key: &Pubkey,
            swap_destination_key: &Pubkey,
            user_destination_key: &Pubkey,
            mut user_destination_account: &mut Account,
            amount_in: u64,
            minimum_amount_out: u64,
            swap_direction: u64,
        ) -> ProgramResult {
            // approve moving from user source account
            let admin_destination_key;

            match swap_direction {
                SWAP_DIRECTION_SELL_BASE => {
                    msg!("swap: swap direction sell base");
                    do_process_instruction(
                        approve(
                            &TOKEN_PROGRAM_ID,
                            &user_source_key,
                            &self.authority_key,
                            &user_key,
                            &[],
                            amount_in,
                        )
                        .unwrap(),
                        vec![
                            &mut user_source_account,
                            &mut Account::default(),
                            &mut Account::default(),
                        ],
                    )
                    .unwrap();

                    admin_destination_key = self.get_admin_fee_key(swap_destination_key);
                }
                SWAP_DIRECTION_SELL_QUOTE => {
                    msg!("swap: swap direction sell quote");
                    do_process_instruction(
                        approve(
                            &TOKEN_PROGRAM_ID,
                            &user_destination_key,
                            &self.authority_key,
                            &user_key,
                            &[],
                            amount_in,
                        )
                        .unwrap(),
                        vec![
                            &mut user_destination_account,
                            &mut Account::default(),
                            &mut Account::default(),
                        ],
                    )
                    .unwrap();

                    admin_destination_key = self.get_admin_fee_key(swap_source_key);
                }
                _ => {
                    admin_destination_key = self.get_admin_fee_key(swap_destination_key);
                }
            }

            let mut admin_destination_account =
                self.get_admin_fee_account(&admin_destination_key).clone();
            let mut swap_source_account = self.get_token_account(swap_source_key).clone();
            let mut swap_destination_account = self.get_token_account(swap_destination_key).clone();

            // perform the swap
            do_process_instruction(
                swap(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &user_source_key,
                    &swap_source_key,
                    &swap_destination_key,
                    &user_destination_key,
                    &self.deltafi_token_key,
                    &self.deltafi_mint_key,
                    &admin_destination_key,
                    amount_in,
                    minimum_amount_out,
                    swap_direction,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut user_source_account,
                    &mut swap_source_account,
                    &mut swap_destination_account,
                    &mut user_destination_account,
                    &mut self.deltafi_token_account,
                    &mut self.deltafi_mint_account,
                    &mut admin_destination_account,
                    &mut Account::default(),
                    &mut clock_account(ZERO_TS),
                ],
            )?;

            self.set_admin_fee_account_(&admin_destination_key, admin_destination_account);
            self.set_token_account(swap_source_key, swap_source_account);
            self.set_token_account(swap_destination_key, swap_destination_account);

            Ok(())
        }

        #[allow(clippy::too_many_arguments)]
        pub fn deposit(
            &mut self,
            depositor_key: &Pubkey,
            depositor_token_a_key: &Pubkey,
            mut depositor_token_a_account: &mut Account,
            depositor_token_b_key: &Pubkey,
            mut depositor_token_b_account: &mut Account,
            depositor_pool_key: &Pubkey,
            mut depositor_pool_account: &mut Account,
            amount_a: u64,
            amount_b: u64,
            min_mint_amount: u64,
        ) -> ProgramResult {
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &depositor_token_a_key,
                    &self.authority_key,
                    &depositor_key,
                    &[],
                    amount_a,
                )
                .unwrap(),
                vec![
                    &mut depositor_token_a_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &depositor_token_b_key,
                    &self.authority_key,
                    &depositor_key,
                    &[],
                    amount_b,
                )
                .unwrap(),
                vec![
                    &mut depositor_token_b_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform deposit
            do_process_instruction(
                deposit(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &depositor_token_a_key,
                    &depositor_token_b_key,
                    &self.token_a_key,
                    &self.token_b_key,
                    &self.pool_mint_key,
                    &depositor_pool_key,
                    amount_a,
                    amount_b,
                    min_mint_amount,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut depositor_token_a_account,
                    &mut depositor_token_b_account,
                    &mut self.token_a_account,
                    &mut self.token_b_account,
                    &mut self.pool_mint_account,
                    &mut depositor_pool_account,
                    &mut Account::default(),
                    &mut clock_account(ZERO_TS),
                ],
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub fn withdraw(
            &mut self,
            user_key: &Pubkey,
            pool_key: &Pubkey,
            mut pool_account: &mut Account,
            token_a_key: &Pubkey,
            mut token_a_account: &mut Account,
            token_b_key: &Pubkey,
            mut token_b_account: &mut Account,
            pool_amount: u64,
            minimum_a_amount: u64,
            minimum_b_amount: u64,
        ) -> ProgramResult {
            // approve swap program to take out pool tokens
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &pool_key,
                    &self.authority_key,
                    &user_key,
                    &[],
                    pool_amount,
                )
                .unwrap(),
                vec![
                    &mut pool_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform withraw
            do_process_instruction(
                withdraw(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.pool_mint_key,
                    &pool_key,
                    &self.token_a_key,
                    &self.token_b_key,
                    &token_a_key,
                    &token_b_key,
                    &self.admin_fee_a_key,
                    &self.admin_fee_b_key,
                    pool_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.pool_mint_account,
                    &mut pool_account,
                    &mut self.token_a_account,
                    &mut self.token_b_account,
                    &mut token_a_account,
                    &mut token_b_account,
                    &mut self.admin_fee_a_account,
                    &mut self.admin_fee_b_account,
                    &mut Account::default(),
                ],
            )?;

            Ok(())
        }

        #[allow(clippy::too_many_arguments)]
        pub fn withdraw_one(
            &mut self,
            user_key: &Pubkey,
            pool_key: &Pubkey,
            mut pool_account: &mut Account,
            dest_token_key: &Pubkey,
            mut dest_token_account: &mut Account,
            pool_amount: u64,
            minimum_amount: u64,
        ) -> ProgramResult {
            // approve swap program to take out pool tokens
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &pool_key,
                    &self.authority_key,
                    &user_key,
                    &[],
                    pool_amount,
                )
                .unwrap(),
                vec![
                    &mut pool_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform withraw_one
            do_process_instruction(
                withdraw_one(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.pool_mint_key,
                    &pool_key,
                    &self.token_a_key,
                    &self.token_b_key,
                    &dest_token_key,
                    &self.admin_fee_a_key,
                    pool_amount,
                    minimum_amount,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.pool_mint_account,
                    &mut pool_account,
                    &mut self.token_a_account,
                    &mut self.token_b_account,
                    &mut dest_token_account,
                    &mut self.admin_fee_a_account,
                    &mut Account::default(),
                    &mut clock_account(ZERO_TS),
                ],
            )
        }

        /** Admin functions **/

        pub fn ramp_a(
            &mut self,
            target_amp: u64,
            current_ts: i64,
            stop_ramp_ts: i64,
        ) -> ProgramResult {
            do_process_instruction(
                ramp_a(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                    target_amp,
                    stop_ramp_ts,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut clock_account(current_ts),
                ],
            )
        }

        pub fn stop_ramp_a(&mut self, current_ts: i64) -> ProgramResult {
            do_process_instruction(
                stop_ramp_a(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut clock_account(current_ts),
                ],
            )
        }

        pub fn pause(&mut self) -> ProgramResult {
            do_process_instruction(
                pause(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                ],
            )
        }

        pub fn unpause(&mut self) -> ProgramResult {
            do_process_instruction(
                unpause(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                ],
            )
        }

        pub fn set_admin_fee_account(
            &mut self,
            new_admin_fee_key: &Pubkey,
            new_admin_fee_account: &Account,
        ) -> ProgramResult {
            do_process_instruction(
                set_fee_account(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                    new_admin_fee_key,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut new_admin_fee_account.clone(),
                ],
            )
        }

        pub fn apply_new_admin(&mut self, current_ts: i64) -> ProgramResult {
            do_process_instruction(
                apply_new_admin(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut clock_account(current_ts),
                ],
            )
        }

        pub fn commit_new_admin(
            &mut self,
            new_admin_key: &Pubkey,
            current_ts: i64,
        ) -> ProgramResult {
            do_process_instruction(
                commit_new_admin(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                    new_admin_key,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut Account::default(),
                    &mut clock_account(current_ts),
                ],
            )
        }

        pub fn set_new_fees(&mut self, new_fees: Fees) -> ProgramResult {
            do_process_instruction(
                set_new_fees(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                    new_fees,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                ],
            )
        }

        pub fn set_new_rewards(&mut self, new_rewards: Rewards) -> ProgramResult {
            do_process_instruction(
                set_rewards(
                    &SWAP_PROGRAM_ID,
                    &self.swap_key,
                    &self.authority_key,
                    &self.admin_key,
                    new_rewards,
                )
                .unwrap(),
                vec![
                    &mut self.swap_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                ],
            )
        }
    }

    pub struct FarmAccountInfo {
        pub nonce: u8,
        pub authority_key: Pubkey,
        pub alloc_point: u64,
        pub reward_unit: u64,
        pub farm_base_key: Pubkey,
        pub farm_base_account: Account,
        pub farm_key: Pubkey,
        pub farm_account: Account,
        pub pool_mint_key: Pubkey,
        pub pool_mint_account: Account,
        pub pool_token_key: Pubkey,
        pub pool_token_account: Account,
        pub token_deltafi_key: Pubkey,
        pub token_deltafi_account: Account,
        pub token_deltafi_mint_key: Pubkey,
        pub token_deltafi_mint_account: Account,
        pub admin_key: Pubkey,
        pub admin_account: Account,
        pub admin_fee_deltafi_key: Pubkey,
        pub admin_fee_deltafi_account: Account,
        pub fees: Fees,
    }

    impl FarmAccountInfo {
        pub fn new(
            mint_owner_key: &Pubkey,
            token_pool_amount: u64,
            alloc_point: u64,
            reward_unit: u64,
            fees: Fees,
        ) -> Self {
            let farm_base_key = pubkey_rand();
            let farm_base_account =
                Account::new(0, FarmBaseInfo::get_packed_len(), &SWAP_PROGRAM_ID);

            let farm_key = pubkey_rand();
            let farm_account = Account::new(0, FarmInfo::get_packed_len(), &SWAP_PROGRAM_ID);
            let (authority_key, nonce) =
                Pubkey::find_program_address(&[&farm_key.to_bytes()[..]], &SWAP_PROGRAM_ID);

            // !! need to fix with real pool account from token swap.
            let (pool_mint_key, mut pool_mint_account) = create_mint(
                &TOKEN_PROGRAM_ID,
                &mint_owner_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let (pool_token_key, pool_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &pool_mint_key,
                &mut pool_mint_account,
                &mint_owner_key,
                &authority_key,
                token_pool_amount,
            );
            let (token_deltafi_mint_key, mut token_deltafi_mint_account) = create_mint(
                &TOKEN_PROGRAM_ID,
                &mint_owner_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let (token_deltafi_key, token_deltafi_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &token_deltafi_mint_key,
                &mut token_deltafi_mint_account,
                &mint_owner_key,
                &authority_key,
                0,
            );
            let (admin_fee_deltafi_key, admin_fee_deltafi_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &token_deltafi_mint_key,
                &mut token_deltafi_mint_account,
                &mint_owner_key,
                &authority_key,
                0,
            );

            let admin_account = Account::default();

            FarmAccountInfo {
                nonce,
                authority_key,
                alloc_point,
                reward_unit,
                farm_base_key,
                farm_base_account,
                farm_key,
                farm_account,
                pool_mint_key,
                pool_mint_account,
                pool_token_key,
                pool_token_account,
                token_deltafi_mint_key,
                token_deltafi_mint_account,
                token_deltafi_key,
                token_deltafi_account,
                admin_key: admin_account.owner,
                admin_account,
                admin_fee_deltafi_key,
                admin_fee_deltafi_account,
                fees,
            }
        }

        pub fn initialize_farm(&mut self, current_ts: i64) -> ProgramResult {
            // msg!("deltafi mint: {:2X?} {:2X?}", self.token_deltafi_mint_key, self.token_deltafi_mint_account);
            do_process_instruction(
                initialize_farm(
                    &SWAP_PROGRAM_ID,
                    &self.farm_base_key,
                    &self.farm_key,
                    &self.authority_key,
                    &self.admin_key,
                    &self.pool_mint_key,
                    &self.token_deltafi_mint_key,
                    self.nonce,
                    self.alloc_point,
                    self.reward_unit,
                )
                .unwrap(),
                vec![
                    &mut self.farm_base_account,
                    &mut self.farm_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut clock_account(current_ts),
                    &mut self.pool_mint_account,
                    &mut self.token_deltafi_mint_account,
                ],
            )
        }

        pub fn apply_new_admin_for_farm(&mut self, current_ts: i64) -> ProgramResult {
            do_process_instruction(
                apply_new_admin_for_farm(
                    &SWAP_PROGRAM_ID,
                    &self.farm_key,
                    &self.authority_key,
                    &self.admin_key,
                )
                .unwrap(),
                vec![
                    &mut self.farm_account,
                    &mut Account::default(),
                    &mut self.admin_account,
                    &mut clock_account(current_ts),
                ],
            )
        }

        pub fn setup_token_accounts(
            &mut self,
            mint_owner: &Pubkey,
            account_owner: &Pubkey,
            lp_amount: u64,
            deltafi_amount: u64,
        ) -> (Pubkey, Account, Pubkey, Account, Pubkey, Account) {
            // please care this as only testing purpose.
            let user_farming_key = pubkey_rand();
            let user_farming_account =
                Account::new(0, FarmingUserInfo::get_packed_len(), &SWAP_PROGRAM_ID);

            let (pool_key, pool_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &self.pool_mint_key,
                &mut self.pool_mint_account,
                &mint_owner,
                &account_owner,
                lp_amount,
            );
            let (deltafi_key, deltafi_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &self.token_deltafi_mint_key,
                &mut self.token_deltafi_mint_account,
                &mint_owner,
                &account_owner,
                deltafi_amount,
            );
            (
                pool_key,
                pool_account,
                deltafi_key,
                deltafi_account,
                user_farming_key,
                user_farming_account,
            )
        }

        pub fn enable_user(
            &mut self,
            user_farming_key: &Pubkey,
            mut user_farming_acount: &mut Account,
        ) -> ProgramResult {
            do_process_instruction(
                farm_enable_user(
                    &SWAP_PROGRAM_ID,
                    &self.farm_key,
                    &self.authority_key,
                    user_farming_key,
                )
                .unwrap(),
                vec![
                    &mut self.farm_account,
                    &mut Account::default(),
                    &mut user_farming_acount,
                ],
            )
        }

        pub fn deposit(
            &mut self,
            depositor_key: &Pubkey,
            depositor_farming_key: &Pubkey,
            mut depositor_farming_account: &mut Account,
            depositor_pool_key: &Pubkey,
            mut depositor_pool_account: &mut Account,
            depositor_deltafi_key: &Pubkey,
            mut depositor_deltafi_account: &mut Account,
            amount_lp: u64,
            min_mint_amount: u64,
        ) -> ProgramResult {
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &depositor_pool_key,
                    &self.authority_key,
                    &depositor_key,
                    &[],
                    amount_lp,
                )
                .unwrap(),
                vec![
                    &mut depositor_pool_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform deposit
            do_process_instruction(
                farm_deposit(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.farm_base_key,
                    &self.farm_key,
                    &self.authority_key,
                    &self.admin_fee_deltafi_key,
                    &depositor_pool_key,
                    &depositor_farming_key,
                    &self.pool_token_key,
                    &self.token_deltafi_mint_key,
                    &depositor_deltafi_key,
                    amount_lp,
                    min_mint_amount,
                )
                .unwrap(),
                vec![
                    &mut self.farm_base_account,
                    &mut self.farm_account,
                    &mut Account::default(),
                    &mut self.admin_fee_deltafi_account,
                    &mut depositor_pool_account,
                    &mut depositor_farming_account,
                    &mut self.pool_token_account,
                    &mut self.token_deltafi_mint_account,
                    &mut depositor_deltafi_account,
                    &mut Account::default(),
                    &mut clock_account(ZERO_TS),
                ],
            )?;

            Ok(())
        }

        pub fn withdraw(
            &mut self,
            user_key: &Pubkey,
            user_farming_key: &Pubkey,
            mut user_farming_account: &mut Account,
            pool_key: &Pubkey,
            mut pool_account: &mut Account,
            withdrawer_deltafi_key: &Pubkey,
            mut withdrawer_deltafi_account: &mut Account,
            amount_lp: u64,
            min_mint_amount: u64,
        ) -> ProgramResult {
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &pool_key,
                    &self.authority_key,
                    &user_key,
                    &[],
                    amount_lp,
                )
                .unwrap(),
                vec![
                    &mut pool_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform withdraw
            do_process_instruction(
                farm_withdraw(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.farm_base_key,
                    &self.farm_key,
                    &self.authority_key,
                    &self.admin_fee_deltafi_key,
                    &pool_key,
                    &user_farming_key,
                    &self.pool_token_key,
                    &self.token_deltafi_mint_key,
                    &withdrawer_deltafi_key,
                    amount_lp,
                    min_mint_amount,
                )
                .unwrap(),
                vec![
                    &mut self.farm_base_account,
                    &mut self.farm_account,
                    &mut Account::default(),
                    &mut self.admin_fee_deltafi_account,
                    &mut pool_account,
                    &mut user_farming_account,
                    &mut self.pool_token_account,
                    &mut self.token_deltafi_mint_account,
                    &mut withdrawer_deltafi_account,
                    &mut Account::default(),
                    &mut clock_account(ZERO_TS),
                ],
            )?;

            Ok(())
        }

        pub fn emergency_withdraw(
            &mut self,
            user_key: &Pubkey,
            user_farming_key: &Pubkey,
            mut user_farming_account: &mut Account,
            pool_key: &Pubkey,
            mut pool_account: &mut Account,
            amount_lp: u64,
            _min_mint_amount: u64,
        ) -> ProgramResult {
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &pool_key,
                    &self.authority_key,
                    &user_key,
                    &[],
                    amount_lp,
                )
                .unwrap(),
                vec![
                    &mut pool_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform deposit
            do_process_instruction(
                farm_emergency_withdraw(
                    &SWAP_PROGRAM_ID,
                    &TOKEN_PROGRAM_ID,
                    &self.farm_key,
                    &self.authority_key,
                    &pool_key,
                    &user_farming_key,
                    &self.pool_token_key,
                )
                .unwrap(),
                vec![
                    &mut self.farm_account,
                    &mut Account::default(),
                    &mut pool_account,
                    &mut user_farming_account,
                    &mut self.pool_token_account,
                    &mut Account::default(),
                    &mut clock_account(ZERO_TS),
                ],
            )?;

            Ok(())
        }

        pub fn print_pending_deltafi(
            &mut self,
            user_key: &Pubkey,
            user_farming_key: &Pubkey,
            mut user_farming_account: &mut Account,
            pool_key: &Pubkey,
            mut pool_account: &mut Account,
            amount_lp: u64,
            _min_mint_amount: u64,
        ) -> ProgramResult {
            do_process_instruction(
                approve(
                    &TOKEN_PROGRAM_ID,
                    &pool_key,
                    &self.authority_key,
                    &user_key,
                    &[],
                    amount_lp,
                )
                .unwrap(),
                vec![
                    &mut pool_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();

            // perform deposit
            do_process_instruction(
                farm_pending_deltafi(
                    &SWAP_PROGRAM_ID,
                    &self.farm_base_key,
                    &self.farm_key,
                    &user_farming_key,
                    &pool_key,
                    &self.pool_mint_key,
                )
                .unwrap(),
                vec![
                    &mut self.farm_base_account,
                    &mut self.farm_account,
                    &mut user_farming_account,
                    &mut pool_account,
                    &mut self.pool_mint_account,
                    &mut clock_account(ZERO_TS),
                ],
            )
        }
    }

    struct TestSyscallStubs {}
    impl program_stubs::SyscallStubs for TestSyscallStubs {
        fn sol_invoke_signed(
            &self,
            instruction: &Instruction,
            account_infos: &[AccountInfo],
            signers_seeds: &[&[&[u8]]],
        ) -> ProgramResult {
            msg!("TestSyscallStubs::sol_invoke_signed()");

            let mut new_account_infos = vec![];

            // mimic check for token program in accounts
            if !account_infos.iter().any(|x| *x.key == TOKEN_PROGRAM_ID) {
                return Err(ProgramError::InvalidAccountData);
            }

            for meta in instruction.accounts.iter() {
                for account_info in account_infos.iter() {
                    if meta.pubkey == *account_info.key {
                        let mut new_account_info = account_info.clone();
                        for seeds in signers_seeds.iter() {
                            let signer =
                                Pubkey::create_program_address(&seeds, &SWAP_PROGRAM_ID).unwrap();
                            if *account_info.key == signer {
                                new_account_info.is_signer = true;
                            }
                        }
                        new_account_infos.push(new_account_info);
                    }
                }
            }

            spl_token::processor::Processor::process(
                &instruction.program_id,
                &new_account_infos,
                &instruction.data,
            )
        }
    }

    fn test_syscall_stubs() {
        use std::sync::Once;
        static ONCE: Once = Once::new();

        ONCE.call_once(|| {
            program_stubs::set_syscall_stubs(Box::new(TestSyscallStubs {}));
        });
    }

    pub fn do_process_instruction(
        instruction: Instruction,
        accounts: Vec<&mut Account>,
    ) -> ProgramResult {
        test_syscall_stubs();
        // approximate the logic in the actual runtime which runs the instruction
        // and only updates accounts if the instruction is successful
        let mut account_clones = accounts.iter().map(|x| (*x).clone()).collect::<Vec<_>>();
        let mut meta = instruction
            .accounts
            .iter()
            .zip(account_clones.iter_mut())
            .map(|(account_meta, account)| (&account_meta.pubkey, account_meta.is_signer, account))
            .collect::<Vec<_>>();
        let mut account_infos = create_is_signer_account_infos(&mut meta);
        let res = if instruction.program_id == SWAP_PROGRAM_ID {
            Processor::process(&instruction.program_id, &account_infos, &instruction.data)
        } else {
            spl_token::processor::Processor::process(
                &instruction.program_id,
                &account_infos,
                &instruction.data,
            )
        };

        if res.is_ok() {
            let mut account_metas = instruction
                .accounts
                .iter()
                .zip(accounts)
                .map(|(account_meta, account)| (&account_meta.pubkey, account))
                .collect::<Vec<_>>();
            for account_info in account_infos.iter_mut() {
                for account_meta in account_metas.iter_mut() {
                    if account_info.key == account_meta.0 {
                        let account = &mut account_meta.1;
                        account.owner = *account_info.owner;
                        account.lamports = **account_info.lamports.borrow();
                        account.data = account_info.data.borrow().to_vec();
                    }
                }
            }
        }
        res
    }

    fn mint_minimum_balance() -> u64 {
        Rent::default().minimum_balance(SplMint::get_packed_len())
    }

    fn account_minimum_balance() -> u64 {
        Rent::default().minimum_balance(SplAccount::get_packed_len())
    }

    pub fn mint_token(
        program_id: &Pubkey,
        mint_key: &Pubkey,
        mut mint_account: &mut Account,
        mint_authority_key: &Pubkey,
        account_owner_key: &Pubkey,
        amount: u64,
    ) -> (Pubkey, Account) {
        let account_key = pubkey_rand();
        let mut account_account = Account::new(
            account_minimum_balance(),
            SplAccount::get_packed_len(),
            &program_id,
        );
        let mut mint_authority_account = Account::default();
        let mut rent_sysvar_account = create_account(&Rent::free(), 1);

        do_process_instruction(
            initialize_account(&program_id, &account_key, &mint_key, account_owner_key).unwrap(),
            vec![
                &mut account_account,
                &mut mint_account,
                &mut mint_authority_account,
                &mut rent_sysvar_account,
            ],
        )
        .unwrap();

        if amount > 0 {
            do_process_instruction(
                mint_to(
                    &program_id,
                    &mint_key,
                    &account_key,
                    &mint_authority_key,
                    &[],
                    amount,
                )
                .unwrap(),
                vec![
                    &mut mint_account,
                    &mut account_account,
                    &mut mint_authority_account,
                ],
            )
            .unwrap();
        }

        (account_key, account_account)
    }

    pub fn create_mint(
        program_id: &Pubkey,
        authority_key: &Pubkey,
        decimals: u8,
        freeze_authority: Option<&Pubkey>,
    ) -> (Pubkey, Account) {
        let mint_key = pubkey_rand();
        let mut mint_account = Account::new(
            mint_minimum_balance(),
            SplMint::get_packed_len(),
            &program_id,
        );
        let mut rent_sysvar_account = create_account(&Rent::free(), 1);

        do_process_instruction(
            initialize_mint(
                &program_id,
                &mint_key,
                authority_key,
                freeze_authority,
                decimals,
            )
            .unwrap(),
            vec![&mut mint_account, &mut rent_sysvar_account],
        )
        .unwrap();

        (mint_key, mint_account)
    }
}
