//! Module for processing admin-only instructions.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
};
use spl_token::instruction::AuthorityType;

use crate::{
    error::SwapError,
    instruction::{AdminInitializeData, AdminInstruction, CommitNewAdmin},
    processor::{
        assert_rent_exempt, assert_uninitialized, authority_id, set_authority, unpack_mint,
        unpack_token_account,
    },
    state::{ConfigInfo, SwapInfo, PROGRAM_VERSION},
    state::{Fees, Rewards},
};

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
        AdminInstruction::CommitNewAdmin(CommitNewAdmin { new_admin_key }) => {
            msg!("Instruction: CommitNewAdmin");
            commit_new_admin(program_id, new_admin_key, accounts)
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
    let market_autority_info = next_account_info(account_info_iter)?;
    let deltafi_mint_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    if !admin_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    assert_rent_exempt(rent, config_info)?;
    let mut config = assert_uninitialized::<ConfigInfo>(config_info)?;
    let (market_autority_key, bump_seed) =
        Pubkey::find_program_address(&[config_info.key.as_ref()], program_id);
    if &market_autority_key != market_autority_info.key {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let token_program_id = *token_program_info.key;
    let deltafi_mint = unpack_mint(deltafi_mint_info, &token_program_id)?;
    if COption::Some(*market_autority_info.key) != deltafi_mint.mint_authority {
        return Err(SwapError::InvalidOwner.into());
    }
    if deltafi_mint.freeze_authority.is_some()
        && deltafi_mint.freeze_authority != COption::Some(*admin_info.key)
    {
        return Err(SwapError::InvalidFreezeAuthority.into());
    }

    config.version = PROGRAM_VERSION;
    config.bump_seed = bump_seed;
    config.admin_key = *admin_info.key;
    config.deltafi_mint = *deltafi_mint_info.key;
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

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
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

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

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
    let token_program_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
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
        return Err(SwapError::IncorrectMint.into());
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Commit new admin (initiate admin transfer)
#[inline(never)]
fn commit_new_admin(
    program_id: &Pubkey,
    new_admin_key: Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let deltafi_mint_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    config.admin_key = new_admin_key;
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;

    set_authority(
        token_program_info,
        deltafi_mint_info,
        Some(new_admin_key),
        AuthorityType::FreezeAccount,
        admin_info,
    )?;

    Ok(())
}

/// Set new fees
#[inline(never)]
fn set_new_fees(program_id: &Pubkey, new_fees: &Fees, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
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

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    token_swap.rewards = Rewards::new(new_rewards);
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}
