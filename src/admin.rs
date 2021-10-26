//! Module for processing admin-only instructions.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::UnixTimestamp,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};

use crate::{
    error::SwapError,
    fees::Fees,
    instruction::{AdminInitializeData, AdminInstruction},
    processor::{authority_id, unpack_token_account},
    rewards::Rewards,
    state::{ConfigInfo, SwapInfo},
};

const ZERO_TS: UnixTimestamp = 0;

/// Process admin instruction
pub fn process_admin_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = AdminInstruction::unpack(input)?;
    match instruction {
        AdminInstruction::Initialize(AdminInitializeData { fees, rewards }) => {
            msg!("AdminInstruction : Initialization");
            initialize(program_id, &fees, &rewards, accounts)
        }
        AdminInstruction::Pause => {
            msg!("Instruction: Pause");
            pause(program_id, accounts)
        }
        AdminInstruction::Unpause => {
            msg!("Instruction: Unpause");
            unpause(program_id, accounts)
        }
        AdminInstruction::SetFeeAccount => {
            msg!("Instruction: SetFeeAccount");
            set_fee_account(program_id, accounts)
        }
        AdminInstruction::ApplyNewAdmin => {
            msg!("Instruction: ApplyNewAdmin");
            apply_new_admin(program_id, accounts)
        }
        AdminInstruction::CommitNewAdmin => {
            msg!("Instruction: CommitNewAdmin");
            commit_new_admin(program_id, accounts)
        }
        AdminInstruction::SetNewFees(new_fees) => {
            msg!("Instruction: SetNewFees");
            set_new_fees(program_id, &new_fees, accounts)
        }
        AdminInstruction::SetNewRewards(new_rewards) => {
            msg!("Instruction: SetRewardsInfo");
            set_new_rewards(program_id, &new_rewards, accounts)
        }
    }
}

/// Access control for admin only instructions
#[inline(never)]
fn is_admin(expected_admin_key: &Pubkey, admin_account_info: &AccountInfo) -> ProgramResult {
    if expected_admin_key != admin_account_info.key {
        return Err(SwapError::Unauthorized.into());
    }
    if !admin_account_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

/// Initialize configuration
#[inline(never)]
fn initialize(
    program_id: &Pubkey,
    fees: &Fees,
    rewards: &Rewards,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;

    let mut config = ConfigInfo::unpack_unchecked(&config_info.data.borrow())?;
    if config.is_initialized {
        return Err(SwapError::AlreadyInUse.into());
    }
    if config_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }

    config.is_initialized = true;
    config.future_admin_key = Pubkey::default();
    config.future_admin_deadline = ZERO_TS;
    config.deltafi_mint = Pubkey::default();
    config.fees = Fees::new(fees);
    config.rewards = Rewards::new(rewards);
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;
    Ok(())
}

/// Pause swap
#[inline(never)]
fn pause(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if swap_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    token_swap.is_paused = true;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Unpause swap
#[inline(never)]
fn unpause(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if swap_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    token_swap.is_paused = false;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set fee account
#[inline(never)]
fn set_fee_account(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let new_fee_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if swap_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    if *authority_info.key != authority_id(program_id, swap_info.key, token_swap.nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let new_admin_fee_account = unpack_token_account(new_fee_account_info, token_program_info.key)?;
    if *authority_info.key != new_admin_fee_account.owner {
        return Err(SwapError::InvalidOwner.into());
    }
    if new_admin_fee_account.mint == token_swap.token_a_mint {
        token_swap.admin_fee_key_a = *new_fee_account_info.key;
    } else if new_admin_fee_account.mint == token_swap.token_b_mint {
        token_swap.admin_fee_key_b = *new_fee_account_info.key;
    } else {
        return Err(SwapError::InvalidAdmin.into());
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Apply new admin (finalize admin transfer)
#[inline(never)]
fn apply_new_admin(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    let mut config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    if config_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    if config.future_admin_deadline == ZERO_TS {
        return Err(SwapError::NoActiveTransfer.into());
    }
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    if clock.unix_timestamp > config.future_admin_deadline {
        return Err(SwapError::AdminDeadlineExceeded.into());
    }

    config.admin_key = config.future_admin_key;
    config.future_admin_key = Pubkey::default();
    config.future_admin_deadline = ZERO_TS;
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;
    Ok(())
}

/// Commit new admin (initiate admin transfer)
#[inline(never)]
fn commit_new_admin(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let new_admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    let mut config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    if config_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    const ADMIN_TRANSFER_DELAY: i64 = 259200;
    if clock.unix_timestamp < config.future_admin_deadline {
        return Err(SwapError::ActiveTransfer.into());
    }

    config.future_admin_key = *new_admin_info.key;
    config.future_admin_deadline = clock
        .unix_timestamp
        .checked_add(ADMIN_TRANSFER_DELAY)
        .ok_or(SwapError::CalculationFailure)?;
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;
    Ok(())
}

/// Set new fees
#[inline(never)]
fn set_new_fees(program_id: &Pubkey, new_fees: &Fees, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if swap_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    token_swap.fees = Fees::new(new_fees);
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set new rewards
#[inline(never)]
fn set_new_rewards(
    program_id: &Pubkey,
    new_rewards: &Rewards,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if swap_info.owner != program_id {
        return Err(SwapError::InvalidOwner.into());
    }
    token_swap.rewards = Rewards::new(new_rewards);
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

#[cfg(feature = "test-bpf")]
mod tests {
    use solana_sdk::{account::Account, clock::Epoch};

    use super::*;
    use crate::{
        curve_1::ZERO_TS,
        utils::{test_utils::*, CURVE_PMM, TWAP_OPENED},
    };

    const DEFAULT_TOKEN_A_AMOUNT: u64 = 1_000_000_000;
    const DEFAULT_TOKEN_B_AMOUNT: u64 = 1_000_000_000;
    const DEFAULT_POOL_TOKEN_AMOUNT: u64 = 0;

    #[test]
    fn test_is_admin() {
        let admin_key = pubkey_rand();
        let admin_owner = pubkey_rand();
        let mut lamports = 0;
        let mut admin_account_data = vec![];
        let mut admin_account_info = AccountInfo::new(
            &admin_key,
            true,
            false,
            &mut lamports,
            &mut admin_account_data,
            &admin_owner,
            false,
            Epoch::default(),
        );

        // Correct admin
        assert_eq!(Ok(()), is_admin(&admin_key, &admin_account_info));

        // Unauthorized account
        let fake_admin_key = pubkey_rand();
        let mut fake_admin_account = admin_account_info.clone();
        fake_admin_account.key = &fake_admin_key;
        assert_eq!(
            Err(SwapError::Unauthorized.into()),
            is_admin(&admin_key, &fake_admin_account)
        );

        // Admin did not sign
        admin_account_info.is_signer = false;
        assert_eq!(
            Err(ProgramError::MissingRequiredSignature),
            is_admin(&admin_key, &admin_account_info)
        );
    }

    #[test]
    fn test_initialize() {
        let amp_factor = MIN_AMP * 100;
        let mut accounts =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);

        // wrong amp_factor
        {
            let old_amp_factor = accounts.amp_factor;
            accounts.amp_factor = MIN_AMP - 1;
            assert_eq!(Err(SwapError::InvalidInput.into()), accounts.initialize());
            accounts.amp_factor = old_amp_factor;
        }

        // Invalid Account Owner
        {
            let old_account = accounts.config_account;
            accounts.config_account = Account::new(0, ConfigInfo::get_packed_len(), &pubkey_rand());
            assert_eq!(Err(SwapError::InvalidOwner.into()), accounts.initialize());
            accounts.config_account = old_account;
        }

        // Valid call
        {
            accounts.initialize().unwrap();
            let config = ConfigInfo::unpack(&accounts.config_account.data).unwrap();
            assert_eq!(config.amp_factor, amp_factor);
            assert_eq!(config.fees, DEFAULT_TEST_FEES);
            assert_eq!(config.rewards, DEFAULT_TEST_REWARDS);
        }
    }

    #[test]
    fn test_ramp_a() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP * 100;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();

        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.ramp_a(MIN_AMP, ZERO_TS, MIN_RAMP_DURATION)
            );
        }

        accounts.initialize_swap().unwrap();

        // Invalid target amp
        {
            let stop_ramp_ts = MIN_RAMP_DURATION;
            let target_amp = 0;
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.ramp_a(target_amp, ZERO_TS, stop_ramp_ts)
            );
            let target_amp = MAX_AMP + 1;
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.ramp_a(target_amp, ZERO_TS, stop_ramp_ts)
            );
        }

        // Unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.ramp_a(MIN_AMP, ZERO_TS, MIN_RAMP_DURATION)
            );
            accounts.admin_key = old_admin_key;
        }

        // ramp locked
        {
            assert_eq!(
                Err(SwapError::RampLocked.into()),
                accounts.ramp_a(MIN_AMP, ZERO_TS, MIN_RAMP_DURATION / 2)
            );
        }

        // insufficient ramp time
        {
            assert_eq!(
                Err(SwapError::InsufficientRampTime.into()),
                accounts.ramp_a(amp_factor, MIN_RAMP_DURATION, ZERO_TS)
            );
        }

        // invalid amp targets
        {
            // amp target too low
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.ramp_a(MIN_AMP, MIN_RAMP_DURATION, MIN_RAMP_DURATION * 2)
            );
            // amp target too high
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.ramp_a(MAX_AMP, MIN_RAMP_DURATION, MIN_RAMP_DURATION * 2)
            );
        }

        // valid ramp
        {
            let target_amp = MIN_AMP * 200;
            let current_ts = MIN_RAMP_DURATION;
            let stop_ramp_ts = MIN_RAMP_DURATION * 2;
            accounts
                .ramp_a(target_amp, current_ts, stop_ramp_ts)
                .unwrap();

            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert_eq!(swap_info.initial_amp_factor, amp_factor);
            assert_eq!(swap_info.target_amp_factor, target_amp);
            assert_eq!(swap_info.start_ramp_ts, current_ts);
            assert_eq!(swap_info.stop_ramp_ts, stop_ramp_ts);
        }
    }

    #[test]
    fn test_stop_ramp_a() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP * 100;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.ramp_a(MIN_AMP, ZERO_TS, MIN_RAMP_DURATION)
            );
        }

        accounts.initialize_swap().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.stop_ramp_a(ZERO_TS)
            );
            accounts.admin_key = old_admin_key;
        }

        // valid call
        {
            let expected_ts = MIN_RAMP_DURATION;
            accounts.stop_ramp_a(expected_ts).unwrap();

            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert_eq!(swap_info.initial_amp_factor, amp_factor);
            assert_eq!(swap_info.target_amp_factor, amp_factor);
            assert_eq!(swap_info.start_ramp_ts, expected_ts);
            assert_eq!(swap_info.stop_ramp_ts, expected_ts);
        }
    }

    #[test]
    fn test_pause() {
        let user_key = pubkey_rand();
        let mut config_account =
            ConfigAccountInfo::new(MIN_AMP, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.pause(),
                "swap not initialized"
            );
        }

        accounts.initialize_swap().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(Err(SwapError::Unauthorized.into()), accounts.pause());
            accounts.admin_key = old_admin_key;
        }

        // valid call
        {
            accounts.pause().unwrap();

            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert!(swap_info.is_paused);
        }
    }

    #[test]
    fn test_unpause() {
        let user_key = pubkey_rand();
        let mut config_account =
            ConfigAccountInfo::new(MIN_AMP, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );

        // swap not initialized
        {
            assert_eq!(Err(ProgramError::UninitializedAccount), accounts.unpause());
        }

        accounts.initialize_swap().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(Err(SwapError::Unauthorized.into()), accounts.unpause());
            accounts.admin_key = old_admin_key;
        }

        // valid call
        {
            // Pause swap pool
            accounts.pause().unwrap();
            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert!(swap_info.is_paused);

            // Unpause swap pool
            accounts.unpause().unwrap();
            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert!(!swap_info.is_paused);
        }
    }

    #[test]
    fn test_set_fee_account() {
        let user_key = pubkey_rand();
        let owner_key = pubkey_rand();
        let amp_factor = MIN_AMP * 100;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );
        let (
            admin_fee_key_a,
            admin_fee_account_a,
            _admin_fee_key_b,
            _admin_fee_account_b,
            wrong_admin_fee_key,
            wrong_admin_fee_account,
        ) = accounts.setup_token_accounts(
            &user_key,
            &owner_key,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_POOL_TOKEN_AMOUNT,
        );

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.set_admin_fee_account(&admin_fee_key_a, &admin_fee_account_a)
            );
        }

        accounts.initialize_swap().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.set_admin_fee_account(&admin_fee_key_a, &admin_fee_account_a)
            );
            accounts.admin_key = old_admin_key;
        }

        // wrong admin account
        {
            assert_eq!(
                Err(SwapError::InvalidOwner.into()),
                accounts.set_admin_fee_account(&wrong_admin_fee_key, &wrong_admin_fee_account)
            );
        }

        // valid calls
        {
            let (
                admin_fee_key_a,
                admin_fee_account_a,
                admin_fee_key_b,
                admin_fee_account_b,
                _wrong_admin_fee_key,
                _wrong_admin_fee_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &accounts.authority_key.clone(),
                DEFAULT_TOKEN_A_AMOUNT,
                DEFAULT_TOKEN_B_AMOUNT,
                DEFAULT_POOL_TOKEN_AMOUNT,
            );
            // set fee account a
            accounts
                .set_admin_fee_account(&admin_fee_key_a, &admin_fee_account_a)
                .unwrap();
            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert_eq!(swap_info.admin_fee_key_a, admin_fee_key_a);
            // set fee acount b
            accounts
                .set_admin_fee_account(&admin_fee_key_b, &admin_fee_account_b)
                .unwrap();
            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert_eq!(swap_info.admin_fee_key_b, admin_fee_key_b);
        }
    }

    #[test]
    fn test_apply_new_admin() {
        let amp_factor = MIN_AMP * 100;
        let mut accounts =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.apply_new_admin(ZERO_TS),
                "swap not initialized",
            );
        }

        accounts.initialize().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.apply_new_admin(ZERO_TS)
            );
            accounts.admin_key = old_admin_key;
        }

        // no active transfer
        {
            assert_eq!(
                Err(SwapError::NoActiveTransfer.into()),
                accounts.apply_new_admin(ZERO_TS)
            );
        }

        // apply new admin
        {
            let new_admin_key = pubkey_rand();
            let current_ts = MIN_RAMP_DURATION;

            // Commit to initiate admin transfer
            accounts
                .commit_new_admin(&new_admin_key, current_ts)
                .unwrap();

            // Applying transfer past deadline should fail
            let apply_deadline = current_ts + MIN_RAMP_DURATION * 3;
            assert_eq!(
                Err(SwapError::AdminDeadlineExceeded.into()),
                accounts.apply_new_admin(apply_deadline + 1)
            );

            // Apply to finalize admin transfer
            accounts.apply_new_admin(current_ts + 1).unwrap();
            let config_info = ConfigInfo::unpack(&accounts.config_account.data).unwrap();
            assert_eq!(config_info.admin_key, new_admin_key);
            assert_eq!(config_info.future_admin_key, Pubkey::default());
            assert_eq!(config_info.future_admin_deadline, ZERO_TS);
        }
    }

    #[test]
    fn test_commit_new_admin() {
        let new_admin_key = pubkey_rand();
        let current_ts = ZERO_TS;
        let amp_factor = MIN_AMP * 100;
        let mut accounts =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.commit_new_admin(&new_admin_key, current_ts)
            );
        }

        accounts.initialize().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.commit_new_admin(&new_admin_key, current_ts)
            );
            accounts.admin_key = old_admin_key;
        }

        // commit new admin
        {
            // valid call
            accounts
                .commit_new_admin(&new_admin_key, current_ts)
                .unwrap();

            let config_info = ConfigInfo::unpack(&accounts.config_account.data).unwrap();
            assert_eq!(config_info.future_admin_key, new_admin_key);
            let expected_future_ts = current_ts + MIN_RAMP_DURATION * 3;
            assert_eq!(config_info.future_admin_deadline, expected_future_ts);

            // new commit within deadline should fail
            assert_eq!(
                Err(SwapError::ActiveTransfer.into()),
                accounts.commit_new_admin(&new_admin_key, current_ts + 1),
            );

            // new commit after deadline should be valid
            let new_admin_key = pubkey_rand();
            let current_ts = expected_future_ts + 1;
            accounts
                .commit_new_admin(&new_admin_key, current_ts)
                .unwrap();
            let config_info = ConfigInfo::unpack(&accounts.config_account.data).unwrap();
            assert_eq!(config_info.future_admin_key, new_admin_key);
            let expected_future_ts = current_ts + MIN_RAMP_DURATION * 3;
            assert_eq!(config_info.future_admin_deadline, expected_future_ts);
        }
    }

    #[test]
    fn test_set_new_fees() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP * 100;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );

        let new_fees: Fees = Fees {
            admin_trade_fee_numerator: 0,
            admin_trade_fee_denominator: 0,
            admin_withdraw_fee_numerator: 0,
            admin_withdraw_fee_denominator: 0,
            trade_fee_numerator: 0,
            trade_fee_denominator: 0,
            withdraw_fee_numerator: 0,
            withdraw_fee_denominator: 0,
        };

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.set_new_fees(new_fees)
            );
        }

        accounts.initialize_swap().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.set_new_fees(new_fees)
            );
            accounts.admin_key = old_admin_key;
        }

        // valid call
        {
            accounts.set_new_fees(new_fees).unwrap();

            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert_eq!(swap_info.fees, new_fees);
        }
    }

    #[test]
    fn test_set_new_rewards() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP * 100;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            default_k(),
            default_i(),
            TWAP_OPENED,
            CURVE_PMM,
        );

        let new_rewards: Rewards = Rewards {
            trade_reward_numerator: 2,
            trade_reward_denominator: 3,
            trade_reward_cap: 100,
            liquidity_reward_numerator: 1,
            liquidity_reward_denominator: 500,
        };

        // swap not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.set_new_rewards(new_rewards)
            );
        }

        accounts.initialize_swap().unwrap();

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.set_new_rewards(new_rewards)
            );
            accounts.admin_key = old_admin_key;
        }

        // valid call
        {
            accounts.set_new_rewards(new_rewards).unwrap();

            let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
            assert_eq!(swap_info.rewards, new_rewards);
        }
    }
}
