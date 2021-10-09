//! Module for processing admin-only instructions.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};

use crate::{
    bn::U256,
    curve::{StableSwap, MAX_AMP, MIN_AMP, MIN_RAMP_DURATION, ZERO_TS},
    error::SwapError,
    fees::Fees,
    instruction::{AdminInitializeData, AdminInstruction, FarmData, RampAData},
    rewards::Rewards,
    state::{ConfigInfo, FarmBaseInfo, FarmInfo, SwapInfo},
    utils,
};

/// Process admin instruction
pub fn process_admin_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = AdminInstruction::unpack(input)?;
    match instruction {
        AdminInstruction::Initialize(AdminInitializeData {
            amp_factor,
            fees,
            rewards,
        }) => {
            msg!("AdminInstruction : Initialization");
            initialize(program_id, amp_factor, &fees, &rewards, accounts)
        }
        AdminInstruction::RampA(RampAData {
            target_amp,
            stop_ramp_ts,
        }) => {
            msg!("Instruction : RampA");
            ramp_a(program_id, target_amp, stop_ramp_ts, accounts)
        }
        AdminInstruction::StopRampA => {
            msg!("Instruction: StopRampA");
            stop_ramp_a(program_id, accounts)
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
        AdminInstruction::InitializeFarm(FarmData {
            nonce,
            alloc_point,
            reward_unit,
        }) => {
            msg!("Instruction: Initialize Farm");
            initialize_farm(program_id, nonce, alloc_point, reward_unit, accounts)
        }
        AdminInstruction::SetFarm(FarmData {
            nonce,
            alloc_point,
            reward_unit,
        }) => {
            msg!("Instruction:: SetFarm");
            set_farm(program_id, nonce, alloc_point, reward_unit, accounts)
        }
        AdminInstruction::ApplyNewAdminForFarm => {
            msg!("Instruction: ApplyNewAdminForFarm");
            apply_new_admin_for_farm(program_id, accounts)
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
    amp_factor: u64,
    fees: &Fees,
    rewards: &Rewards,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let deltafi_mint_info = next_account_info(account_info_iter)?;

    let mut config = ConfigInfo::unpack_unchecked(&config_info.data.borrow())?;
    if config.is_initialized {
        return Err(SwapError::AlreadyInUse.into());
    }
    if !admin_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let (config_authority, _) =
        Pubkey::find_program_address(&[&config_info.key.to_bytes()], program_id);
    if *authority_info.key != config_authority {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    if !(MIN_AMP..=MAX_AMP).contains(&amp_factor) {
        return Err(SwapError::InvalidInput.into());
    }

    config.is_initialized = true;
    config.amp_factor = amp_factor;
    config.admin_key = *admin_info.key;
    config.future_admin_key = Pubkey::default();
    config.future_admin_deadline = ZERO_TS;
    config.deltafi_mint = *deltafi_mint_info.key;
    config.fees = *fees;
    config.rewards = *rewards;
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;
    Ok(())
}

/// Ramp to future a
#[inline(never)]
fn ramp_a(
    _program_id: &Pubkey,
    target_amp: u64,
    stop_ramp_ts: i64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    if !(MIN_AMP..=MAX_AMP).contains(&target_amp) {
        return Err(SwapError::InvalidInput.into());
    }
    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    let ramp_lock_ts = token_swap
        .start_ramp_ts
        .checked_add(MIN_RAMP_DURATION)
        .ok_or(SwapError::CalculationFailure)?;
    if clock.unix_timestamp < ramp_lock_ts {
        return Err(SwapError::RampLocked.into());
    }
    let min_ramp_ts = clock
        .unix_timestamp
        .checked_add(MIN_RAMP_DURATION)
        .ok_or(SwapError::CalculationFailure)?;
    if stop_ramp_ts < min_ramp_ts {
        return Err(SwapError::InsufficientRampTime.into());
    }

    const MAX_A_CHANGE: u64 = 10;
    let invariant = StableSwap::new(
        token_swap.initial_amp_factor,
        token_swap.target_amp_factor,
        clock.unix_timestamp,
        token_swap.start_ramp_ts,
        token_swap.stop_ramp_ts,
    );
    let current_amp = U256::to_u64(
        invariant
            .compute_amp_factor()
            .ok_or(SwapError::CalculationFailure)?,
    )?;
    if target_amp < current_amp {
        if current_amp > target_amp * MAX_A_CHANGE {
            // target_amp too low
            return Err(SwapError::InvalidInput.into());
        }
    } else if target_amp > current_amp * MAX_A_CHANGE {
        // target_amp too high
        return Err(SwapError::InvalidInput.into());
    }

    token_swap.initial_amp_factor = current_amp;
    token_swap.target_amp_factor = target_amp;
    token_swap.start_ramp_ts = clock.unix_timestamp;
    token_swap.stop_ramp_ts = stop_ramp_ts;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Stop ramp a
#[inline(never)]
fn stop_ramp_a(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    let invariant = StableSwap::new(
        token_swap.initial_amp_factor,
        token_swap.target_amp_factor,
        clock.unix_timestamp,
        token_swap.start_ramp_ts,
        token_swap.stop_ramp_ts,
    );
    let current_amp = U256::to_u64(
        invariant
            .compute_amp_factor()
            .ok_or(SwapError::CalculationFailure)?,
    )?;

    token_swap.initial_amp_factor = current_amp;
    token_swap.target_amp_factor = current_amp;
    token_swap.start_ramp_ts = clock.unix_timestamp;
    token_swap.stop_ramp_ts = clock.unix_timestamp;
    // now (current_ts < stop_ramp_ts) is always False, compute_amp_factor should return target_amp
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Pause swap
#[inline(never)]
fn pause(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    token_swap.is_paused = true;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Unpause swap
#[inline(never)]
fn unpause(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
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

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if *authority_info.key != utils::authority_id(program_id, swap_info.key, token_swap.nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let new_admin_fee_account = utils::unpack_token_account(&new_fee_account_info.data.borrow())?;
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
fn apply_new_admin(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    let mut config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

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
fn commit_new_admin(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let new_admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    let mut config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

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
fn set_new_fees(_program_id: &Pubkey, new_fees: &Fees, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    token_swap.fees = *new_fees;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set new rewards
#[inline(never)]
fn set_new_rewards(
    _program_id: &Pubkey,
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
    token_swap.rewards = *new_rewards;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Processes an [Farm's Initialize](enum.Instruction.html).
/// I should to consider whether something to initialize for farm is, and
/// maybe it depends on farm structure.
pub fn initialize_farm(
    program_id: &Pubkey,
    nonce: u8,
    alloc_point: u64,
    reward_unit: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let farm_base_info = next_account_info(account_info_iter)?;
    let farm_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let deltafi_mint_info = next_account_info(account_info_iter)?;

    let mut farm = FarmInfo::unpack_unchecked(&farm_info.data.borrow())?;
    let mut farm_base = FarmBaseInfo::unpack_unchecked(&farm_base_info.data.borrow())?;
    is_admin(&farm.admin_key, admin_info)?;
    if *authority_info.key != utils::authority_id(program_id, farm_info.key, nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let clock = Clock::from_account_info(clock_sysvar_info)?;

    farm_base.is_initialized = true;
    farm_base.total_alloc_point += alloc_point;
    farm_base.reward_unit = reward_unit;
    farm.is_initialized = true;
    farm.alloc_point = alloc_point;
    farm.acc_deltafi_per_share = 0;
    farm.last_reward_timestamp = clock.unix_timestamp;
    farm.pool_mint = *pool_mint_info.key;
    farm.token_deltafi_mint = *deltafi_mint_info.key;
    farm.nonce = nonce;

    // !!initialize other properties
    // ...

    FarmBaseInfo::pack(farm_base, &mut farm_base_info.data.borrow_mut())?;
    FarmInfo::pack(farm, &mut farm_info.data.borrow_mut())?;
    // msg!("at initialize_farm, farm_info: {:2X?}", farm_info);
    Ok(())
}

/// Apply new admin for farm (finalize admin transfer)
fn apply_new_admin_for_farm(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let farm_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    // msg!("at apply_new_admin_for_farm, farm_info: {:2X?}", farm_info);
    let mut farm = FarmInfo::unpack(&farm_info.data.borrow())?;
    is_admin(&farm.admin_key, admin_info)?;
    if *authority_info.key != utils::authority_id(program_id, farm_info.key, farm.nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if farm.future_admin_deadline == ZERO_TS {
        return Err(SwapError::NoActiveTransfer.into());
    }
    let clock = Clock::from_account_info(clock_sysvar_info)?;
    if clock.unix_timestamp > farm.future_admin_deadline {
        return Err(SwapError::AdminDeadlineExceeded.into());
    }

    farm.admin_key = farm.future_admin_key;
    farm.future_admin_key = Pubkey::default();
    farm.future_admin_deadline = ZERO_TS;
    FarmInfo::pack(farm, &mut farm_info.data.borrow_mut())?;
    Ok(())
}

/// set farm with already initialized one.
pub fn set_farm(
    program_id: &Pubkey,
    nonce: u8,
    alloc_point: u64,
    _reward_unit: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let farm_base_info = next_account_info(account_info_iter)?;
    let farm_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    let mut farm = FarmInfo::unpack(&farm_info.data.borrow())?;
    let mut farm_base = FarmBaseInfo::unpack(&farm_base_info.data.borrow())?;
    is_admin(&farm.admin_key, admin_info)?;
    if *authority_info.key != utils::authority_id(program_id, farm_info.key, nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    farm_base.total_alloc_point += alloc_point;
    farm.alloc_point = alloc_point;
    FarmBaseInfo::pack(farm_base, &mut farm_base_info.data.borrow_mut())?;
    FarmInfo::pack(farm, &mut farm_info.data.borrow_mut())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use solana_sdk::clock::Epoch;

    use super::*;
    use crate::{
        curve::ZERO_TS,
        utils::{test_utils::*, TWAP_OPENED},
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

        // wrong authority
        {
            let old_authority_key = accounts.authority_key;
            let (authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.config_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.initialize()
            );
            accounts.authority_key = old_authority_key;
        }

        // wrong amp_factor
        {
            let old_amp_factor = accounts.amp_factor;
            accounts.amp_factor = MIN_AMP - 1;
            assert_eq!(Err(SwapError::InvalidInput.into()), accounts.initialize());
            accounts.amp_factor = old_amp_factor;
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
            amp_factor,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
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
            amp_factor,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
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
            MIN_AMP,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
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
            MIN_AMP,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
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
            amp_factor,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
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
            amp_factor,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
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
            amp_factor,
            DEFAULT_TOKEN_A_AMOUNT,
            DEFAULT_TOKEN_B_AMOUNT,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
            TWAP_OPENED,
        );

        let new_rewards: Rewards = Rewards {
            trade_reward_numerator: 2,
            trade_reward_denominator: 3,
            trade_reward_cap: 100,
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

    #[test]
    fn test_initialize_farm() {
        let user_key = pubkey_rand();
        let token_pool_amount = 1000;
        let alloc_point = 200;
        let reward_unit = 10;
        let mut accounts = FarmAccountInfo::new(
            &user_key,
            token_pool_amount,
            alloc_point,
            reward_unit,
            DEFAULT_TEST_FEES,
        );

        assert_eq!(
            accounts.initialize_farm(ZERO_TS).ok(),
            Some(()),
            "intialize farm"
        );

        // wrong nonce for authority_key
        {
            let old_authority = accounts.authority_key;
            let (bad_authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.farm_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = bad_authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.apply_new_admin_for_farm(ZERO_TS),
                "wrong nonce for authority_key",
            );
            accounts.authority_key = old_authority;
        }

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.apply_new_admin_for_farm(ZERO_TS),
                "unauthorized account"
            );
            accounts.admin_key = old_admin_key;
        }
    }

    #[test]
    fn test_set_farm() {
        let user_key = pubkey_rand();
        let token_pool_amount = 1000;
        let alloc_point = 200;
        let reward_unit = 10;
        let mut accounts = FarmAccountInfo::new(
            &user_key,
            token_pool_amount,
            alloc_point,
            reward_unit,
            DEFAULT_TEST_FEES,
        );

        // farm not initialized
        {
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.apply_new_admin_for_farm(ZERO_TS)
            );
        }

        accounts.initialize_farm(ZERO_TS).unwrap();

        // wrong nonce for authority_key
        {
            let old_authority = accounts.authority_key;
            let (bad_authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.farm_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = bad_authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.apply_new_admin_for_farm(ZERO_TS)
            );
            accounts.authority_key = old_authority;
        }

        // unauthorized account
        {
            let old_admin_key = accounts.admin_key;
            let fake_admin_key = pubkey_rand();
            accounts.admin_key = fake_admin_key;
            assert_eq!(
                Err(SwapError::Unauthorized.into()),
                accounts.apply_new_admin_for_farm(ZERO_TS)
            );
            accounts.admin_key = old_admin_key;
        }

        // initialize
        {}
    }
}
