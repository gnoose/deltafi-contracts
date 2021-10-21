//! Instruction types

#![allow(clippy::too_many_arguments)]

use std::{convert::TryInto, mem::size_of};

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::{Pubkey, PUBKEY_BYTES},
    sysvar::clock,
};

use crate::{error::SwapError, fees::Fees, rewards::Rewards};

/// Instruction Type
#[repr(C)]
pub enum InstructionType {
    /// Admin
    Admin,
    /// Swap
    Swap,
    /// Farm
    Farm,
}

impl InstructionType {
    #[doc(hidden)]
    pub fn check(input: &[u8]) -> Option<Self> {
        let (&tag, _rest) = input.split_first()?;
        match tag {
            100..=112 => Some(Self::Admin),
            0u8..=5u8 => Some(Self::Swap),
            0x1Eu8..=0x22u8 => Some(Self::Farm),
            _ => None,
        }
    }
}

/// SWAP INSTRUNCTION DATA
/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct InitializeData {
    /// Nonce used to create valid program address
    pub nonce: u8,
    /// Slope variable - real value * 10**6
    pub k: u64,
    /// mid price 0 ~ 10**6
    pub i: u64,
    /// flag to know about twap open
    pub is_open_twap: u8,
    /// flag to know about curve mode
    pub curve_mode: u8,
}

/// Swap instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct SwapData {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    pub amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    pub minimum_amount_out: u64,
    /// Swap direction 0 -> Sell Base Token, 1 -> Sell Quote Token
    pub swap_direction: u8,
    /// flag to know about curve mode
    pub curve_mode: u8,
}

/// Deposit instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositData {
    /// Token A amount to deposit
    pub token_a_amount: u64,
    /// Token B amount to deposit
    pub token_b_amount: u64,
    /// Minimum LP tokens to mint, prevents excessive slippage
    pub min_mint_amount: u64,
    /// flag to know about curve mode
    pub curve_mode: u8,
}

/// Withdraw instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawData {
    /// Amount of pool tokens to burn. User receives an output of token a
    /// and b based on the percentage of the pool tokens that are returned.
    pub pool_token_amount: u64,
    /// Minimum amount of token A to receive, prevents excessive slippage
    pub minimum_token_a_amount: u64,
    /// Minimum amount of token B to receive, prevents excessive slippage
    pub minimum_token_b_amount: u64,
}

/// Withdraw instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawOneData {
    /// Amount of pool tokens to burn. User receives an output of token a
    /// or b based on the percentage of the pool tokens that are returned.
    pub pool_token_amount: u64,
    /// Minimum amount of token A or B to receive, prevents excessive slippage
    pub minimum_token_amount: u64,
}

/// ADMIN INSTRUCTION DATA
/// Admin initialize config data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct AdminInitializeData {
    /// Default amp coefficient
    pub amp_factor: u64,
    /// Default fees
    pub fees: Fees,
    /// Default rewards
    pub rewards: Rewards,
}
/// RampA instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct RampAData {
    /// Amp. Coefficient to ramp to
    pub target_amp: u64,
    /// Unix timestamp to stop ramp
    pub stop_ramp_ts: i64,
}
/// Farm Initialize instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct FarmData {
    /// Nonce used to create valid program address
    pub nonce: u8,
    /// alloc point for farm
    pub alloc_point: u64,
    /// reward unit for farm
    pub reward_unit: u64,
}

/// FARM INSTRUCTION DATA

/// Farm Deposit instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct FarmingWithdrawData {
    /// Amount of pool tokens to withdraw.
    pub pool_token_amount: u64,
    /// Minimum amount of LP token to receive, prevents excessive slippage
    pub min_pool_token_amount: u64,
}

/// Farm Deposit instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct FarmingDepositData {
    /// LP token amount to deposit
    pub pool_token_amount: u64,
    // / Minimum detafi tokens to mint, prevents excessive slippage
    // pub min_mint_amount: u64,
}

/// Admin only instructions.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum AdminInstruction {
    /// Admin initialization instruction
    Initialize(AdminInitializeData),
    /// TODO: Docs
    RampA(RampAData),
    /// TODO: Docs
    StopRampA,
    /// TODO: Docs
    Pause,
    /// TODO: Docs
    Unpause,
    /// TODO: Docs
    SetFeeAccount,
    /// TODO: Docs
    ApplyNewAdmin,
    /// TODO: Docs
    CommitNewAdmin,
    /// TODO: Docs
    SetNewFees(Fees),
    /// Add new farm with alloc point.
    InitializeFarm(FarmData),
    /// Set alloc point to farm.
    SetFarm(FarmData),
    /// TODO: Docs
    ApplyNewAdminForFarm,
    /// TODO: Docs
    SetNewRewards(Rewards),
}

impl AdminInstruction {
    /// Unpacks a byte buffer into a [AdminInstruction](enum.AdminInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            100 => {
                let (amp_factor, rest) = unpack_u64(rest)?;
                let (fees, rest) = rest.split_at(Fees::LEN);
                let fees = Fees::unpack_unchecked(fees)?;
                let (rewards, _rest) = rest.split_at(Rewards::LEN);
                let rewards = Rewards::unpack_unchecked(rewards)?;
                Self::Initialize(AdminInitializeData {
                    amp_factor,
                    fees,
                    rewards,
                })
            }
            101 => {
                let (target_amp, rest) = unpack_u64(rest)?;
                let (stop_ramp_ts, _rest) = unpack_i64(rest)?;
                Self::RampA(RampAData {
                    target_amp,
                    stop_ramp_ts,
                })
            }
            102 => Self::StopRampA,
            103 => Self::Pause,
            104 => Self::Unpause,
            105 => Self::SetFeeAccount,
            106 => Self::ApplyNewAdmin,
            107 => Self::CommitNewAdmin,
            108 => {
                let fees = Fees::unpack_unchecked(rest)?;
                Self::SetNewFees(fees)
            }
            109 => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (alloc_point, rest) = unpack_u64(rest)?;
                let (reward_unit, _rest) = unpack_u64(rest)?;
                Self::InitializeFarm(FarmData {
                    nonce,
                    alloc_point,
                    reward_unit,
                })
            }
            110 => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (alloc_point, rest) = unpack_u64(rest)?;
                let (reward_unit, _rest) = unpack_u64(rest)?;
                Self::SetFarm(FarmData {
                    nonce,
                    alloc_point,
                    reward_unit,
                })
            }
            111 => Self::ApplyNewAdminForFarm,
            112 => {
                let rewards = Rewards::unpack_unchecked(rest)?;
                Self::SetNewRewards(rewards)
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [AdminInstruction](enum.AdminInstruciton.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(AdminInitializeData {
                amp_factor,
                fees,
                rewards,
            }) => {
                buf.push(100);
                buf.extend_from_slice(&amp_factor.to_le_bytes());
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(&fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(&rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
            Self::RampA(RampAData {
                target_amp,
                stop_ramp_ts,
            }) => {
                buf.push(101);
                buf.extend_from_slice(&target_amp.to_le_bytes());
                buf.extend_from_slice(&stop_ramp_ts.to_le_bytes());
            }
            Self::StopRampA => buf.push(102),
            Self::Pause => buf.push(103),
            Self::Unpause => buf.push(104),
            Self::SetFeeAccount => buf.push(105),
            Self::ApplyNewAdmin => buf.push(106),
            Self::CommitNewAdmin => buf.push(107),
            Self::SetNewFees(fees) => {
                buf.push(108);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(&fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
            }
            Self::InitializeFarm(FarmData {
                nonce,
                alloc_point,
                reward_unit,
            }) => {
                buf.push(109);
                buf.push(nonce);
                buf.extend_from_slice(&alloc_point.to_le_bytes());
                buf.extend_from_slice(&reward_unit.to_le_bytes());
            }
            Self::SetFarm(FarmData {
                nonce,
                alloc_point,
                reward_unit,
            }) => {
                buf.push(110);
                buf.push(nonce);
                buf.extend_from_slice(&alloc_point.to_le_bytes());
                buf.extend_from_slice(&reward_unit.to_le_bytes());
            }
            Self::ApplyNewAdminForFarm => buf.push(111),
            Self::SetNewRewards(rewards) => {
                buf.push(112);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(&rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction
pub fn initialize_config(
    program_id: &Pubkey,
    config_key: &Pubkey,
    admin_key: &Pubkey,
    amp_factor: u64,
    fees: Fees,
    rewards: Rewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Initialize(AdminInitializeData {
        amp_factor,
        fees,
        rewards,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*config_key, true),
        AccountMeta::new_readonly(*admin_key, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'ramp_a' instruction
pub fn ramp_a(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    target_amp: u64,
    stop_ramp_ts: i64,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::RampA(RampAData {
        target_amp,
        stop_ramp_ts,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
        AccountMeta::new_readonly(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'stop_ramp_a' instruction
pub fn stop_ramp_a(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::StopRampA.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
        AccountMeta::new_readonly(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'pause' instruction
pub fn pause(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Pause.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'unpause' instruction
pub fn unpause(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Unpause.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'apply_new_admin' instruction
pub fn apply_new_admin(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::ApplyNewAdmin.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
        AccountMeta::new_readonly(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'apply_new_admin_for_farm' instruction
pub fn apply_new_admin_for_farm(
    program_id: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::ApplyNewAdminForFarm.pack();

    let accounts = vec![
        AccountMeta::new(*farm_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'commit_new_admin' instruction
pub fn commit_new_admin(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::CommitNewAdmin.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
        AccountMeta::new_readonly(*new_admin_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_fee_account' instruction
pub fn set_fee_account(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_fee_account_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetFeeAccount.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*admin_pubkey, true),
        AccountMeta::new_readonly(*new_fee_account_pubkey, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_new_fees' instruction
pub fn set_new_fees(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_fees: Fees,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewFees(new_fees).pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, false),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_rewards' instruction.
pub fn set_rewards(
    program_id: &Pubkey,
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_rewards: Rewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewRewards(new_rewards).pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, true),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'initialize_farm' instruction
pub fn initialize_farm(
    program_id: &Pubkey,
    farm_base_pubkey: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    deltafi_mint_pubkey: &Pubkey,
    nonce: u8,
    alloc_point: u64,
    reward_unit: u64,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::InitializeFarm(FarmData {
        nonce,
        alloc_point,
        reward_unit,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*farm_base_pubkey, false),
        AccountMeta::new(*farm_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
        AccountMeta::new(clock::id(), false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*deltafi_mint_pubkey, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_farm' instruction
pub fn set_farm(
    program_id: &Pubkey,
    farm_base_pubkey: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    nonce: u8,
    alloc_point: u64,
    reward_unit: u64,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::InitializeFarm(FarmData {
        nonce,
        alloc_point,
        reward_unit,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*farm_base_pubkey, false),
        AccountMeta::new(*farm_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Instructions supported by the pmm SwapInfo program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum SwapInstruction {
    ///   Initializes a new SwapInfo.
    ///
    ///   0. `[writable, signer]` New Token-swap to create.
    ///   1. `[]` $authority derived from `create_program_address(&[Token-swap account])`
    ///   2. `[]` admin Account.
    ///   3. `[]` admin_fee_a admin fee Account for token_a.
    ///   4. `[]` admin_fee_b admin fee Account for token_b.
    ///   5. `[]` token_a Account. Must be non zero, owned by $authority.
    ///   6. `[]` token_b Account. Must be non zero, owned by $authority.
    ///   7. `[writable]` Pool Token Mint. Must be empty, owned by $authority.
    Initialize(InitializeData),

    ///   Swap the tokens in the pool.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` token_(A|B) SOURCE Account, amount is transferable by $authority,
    ///   3. `[writable]` token_(A|B) Base Account to swap INTO.  Must be the SOURCE token.
    ///   4. `[writable]` token_(A|B) Base Account to swap FROM.  Must be the DESTINATION token.
    ///   5. `[writable]` token_(A|B) DESTINATION Account assigned to USER as the owner.
    ///   6. `[writable]` token_(A|B) admin fee Account. Must have same mint as DESTINATION token.
    ///   7. `[]` Token program id
    ///   8. `[]` Clock sysvar
    Swap(SwapData),

    ///   Deposit some tokens into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` token_a $authority can transfer amount,
    ///   3. `[writable]` token_b $authority can transfer amount,
    ///   4. `[writable]` token_a Base Account to deposit into.
    ///   5. `[writable]` token_b Base Account to deposit into.
    ///   6. `[writable]` Pool MINT account, $authority is the owner.
    ///   7. `[writable]` Pool Account to deposit the generated tokens, user is the owner.
    ///   8. `[]` Token program id
    ///   9. `[]` Clock sysvar
    Deposit(DepositData),

    ///   Withdraw tokens from the pool at the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` Pool mint account, $authority is the owner
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    ///   4. `[writable]` token_a Swap Account to withdraw FROM.
    ///   5. `[writable]` token_b Swap Account to withdraw FROM.
    ///   6. `[writable]` token_a user Account to credit.
    ///   7. `[writable]` token_b user Account to credit.
    ///   8. `[writable]` admin_fee_a admin fee Account for token_a.
    ///   9. `[writable]` admin_fee_b admin fee Account for token_b.
    ///   10. `[]` Token program id
    Withdraw(WithdrawData),

    ///   Withdraw one token from the pool at the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` Pool mint account, $authority is the owner
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    ///   4. `[writable]` token_(A|B) BASE token Swap Account to withdraw FROM.
    ///   5. `[writable]` token_(A|B) QUOTE token Swap Account to exchange to base token.
    ///   6. `[writable]` token_(A|B) BASE token user Account to credit.
    ///   7. `[writable]` token_(A|B) admin fee Account. Must have same mint as BASE token.
    ///   8. `[]` Token program id
    ///   9. `[]` Clock sysvar
    WithdrawOne(WithdrawOneData),

    ///   Calc the receive amount in the pool - pmm.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` token_(A|B) SOURCE Account, amount is transferable by $authority,
    ///   3. `[writable]` token_(A|B) Base Account to swap INTO.  Must be the SOURCE token.
    ///   4. `[writable]` token_(A|B) Base Account to swap FROM.  Must be the DESTINATION token.
    ///   5. `[writable]` token_(A|B) DESTINATION Account assigned to USER as the owner.
    ///   6. `[writable]` token_(A|B) admin fee Account. Must have same mint as DESTINATION token.
    ///   7. `[]` Token program id
    ///   8. `[]` Clock sysvar
    CalcReceiveAmount(SwapData),
}

impl SwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Option<Self>, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            0x0 => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (k, rest) = unpack_u64(rest)?;
                let (i, rest) = unpack_u64(rest)?;
                let (is_open_twap, rest) = unpack_u8(rest)?;
                let (curve_mode, _rest) = unpack_u8(rest)?;
                Some(Self::Initialize(InitializeData {
                    nonce,
                    k,
                    i,
                    is_open_twap,
                    curve_mode,
                }))
            }
            0x1 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, rest) = unpack_u64(rest)?;
                let (swap_direction, rest) = unpack_u8(rest)?;
                let (curve_mode, _rest) = unpack_u8(rest)?;
                Some(Self::Swap(SwapData {
                    amount_in,
                    minimum_amount_out,
                    swap_direction,
                    curve_mode,
                }))
            }
            0x2 => {
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (min_mint_amount, rest) = unpack_u64(rest)?;
                let (curve_mode, _rest) = unpack_u8(rest)?;
                Some(Self::Deposit(DepositData {
                    token_a_amount,
                    token_b_amount,
                    min_mint_amount,
                    curve_mode,
                }))
            }
            0x3 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_b_amount, _rest) = unpack_u64(rest)?;
                Some(Self::Withdraw(WithdrawData {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                }))
            }
            0x4 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_amount, _rest) = unpack_u64(rest)?;
                Some(Self::WithdrawOne(WithdrawOneData {
                    pool_token_amount,
                    minimum_token_amount,
                }))
            }
            0x5 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, rest) = unpack_u64(rest)?;
                let (swap_direction, rest) = unpack_u8(rest)?;
                let (curve_mode, _rest) = unpack_u8(rest)?;
                Some(Self::CalcReceiveAmount(SwapData {
                    amount_in,
                    minimum_amount_out,
                    swap_direction,
                    curve_mode,
                }))
            }
            _ => None,
        })
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(InitializeData {
                nonce,
                k,
                i,
                is_open_twap,
                curve_mode,
            }) => {
                buf.push(0x0);
                buf.push(nonce);
                buf.extend_from_slice(&k.to_le_bytes());
                buf.extend_from_slice(&i.to_le_bytes());
                buf.extend_from_slice(&is_open_twap.to_le_bytes());
                buf.extend_from_slice(&curve_mode.to_le_bytes());
            }
            Self::Swap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
                curve_mode,
            }) => {
                buf.push(0x1);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
                buf.extend_from_slice(&swap_direction.to_le_bytes());
                buf.extend_from_slice(&curve_mode.to_le_bytes());
            }
            Self::Deposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
                curve_mode,
            }) => {
                buf.push(0x2);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
                buf.extend_from_slice(&min_mint_amount.to_le_bytes());
                buf.extend_from_slice(&curve_mode.to_le_bytes());
            }
            Self::Withdraw(WithdrawData {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                buf.push(0x3);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            }
            Self::WithdrawOne(WithdrawOneData {
                pool_token_amount,
                minimum_token_amount,
            }) => {
                buf.push(0x4);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_amount.to_le_bytes());
            }
            Self::CalcReceiveAmount(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
                curve_mode,
            }) => {
                buf.push(0x5);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
                buf.extend_from_slice(&swap_direction.to_le_bytes());
                buf.extend_from_slice(&curve_mode.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    token_program_id: &Pubkey, // Token program used for the pool token
    config_pubkey: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_fee_a_pubkey: &Pubkey,
    admin_fee_b_pubkey: &Pubkey,
    token_a_pubkey: &Pubkey,
    token_b_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    destination_pubkey: &Pubkey, // Desintation to mint pool tokens for bootstrapper
    deltafi_token_pubkey: &Pubkey,
    pyth_key: &Pubkey,
    nonce: u8,
    k: u64,
    i: u64,
    is_open_twap: u8,
    curve_mode: u8,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Initialize(InitializeData {
        nonce,
        k,
        i,
        is_open_twap,
        curve_mode,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new_readonly(*config_pubkey, false),
        AccountMeta::new_readonly(*swap_pubkey, true),
        AccountMeta::new_readonly(*authority_pubkey, false),
        AccountMeta::new_readonly(*admin_fee_a_pubkey, false),
        AccountMeta::new_readonly(*admin_fee_b_pubkey, false),
        AccountMeta::new_readonly(*token_a_pubkey, false),
        AccountMeta::new_readonly(*token_b_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new_readonly(*deltafi_token_pubkey, false),
        AccountMeta::new_readonly(*pyth_key, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'deposit' instruction.
pub fn deposit(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    deposit_token_a_pubkey: &Pubkey,
    deposit_token_b_pubkey: &Pubkey,
    swap_token_a_pubkey: &Pubkey,
    swap_token_b_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    pyth_key: &Pubkey,
    token_a_amount: u64,
    token_b_amount: u64,
    min_mint_amount: u64,
    curve_mode: u8,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Deposit(DepositData {
        token_a_amount,
        token_b_amount,
        min_mint_amount,
        curve_mode,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*deposit_token_a_pubkey, false),
        AccountMeta::new(*deposit_token_b_pubkey, false),
        AccountMeta::new(*swap_token_a_pubkey, false),
        AccountMeta::new(*swap_token_b_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*pyth_key, false),
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw' instruction.
pub fn withdraw(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    swap_token_a_pubkey: &Pubkey,
    swap_token_b_pubkey: &Pubkey,
    destination_token_a_pubkey: &Pubkey,
    destination_token_b_pubkey: &Pubkey,
    admin_fee_a_pubkey: &Pubkey,
    admin_fee_b_pubkey: &Pubkey,
    pool_token_amount: u64,
    minimum_token_a_amount: u64,
    minimum_token_b_amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Withdraw(WithdrawData {
        pool_token_amount,
        minimum_token_a_amount,
        minimum_token_b_amount,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*swap_token_a_pubkey, false),
        AccountMeta::new(*swap_token_b_pubkey, false),
        AccountMeta::new(*destination_token_a_pubkey, false),
        AccountMeta::new(*destination_token_b_pubkey, false),
        AccountMeta::new(*admin_fee_a_pubkey, false),
        AccountMeta::new(*admin_fee_b_pubkey, false),
        AccountMeta::new(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'calc_receive_amount' instruction.
pub fn calc_receive_amount(
    program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    swap_source_pubkey: &Pubkey,
    swap_destination_pubkey: &Pubkey,
    reward_token_pubkey: &Pubkey,
    reward_mint_pubkey: &Pubkey,
    admin_fee_destination_pubkey: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    swap_direction: u8,
    curve_mode: u8,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::CalcReceiveAmount(SwapData {
        amount_in,
        minimum_amount_out,
        swap_direction,
        curve_mode,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*swap_source_pubkey, false),
        AccountMeta::new(*swap_destination_pubkey, false),
        AccountMeta::new(*reward_token_pubkey, false),
        AccountMeta::new(*reward_mint_pubkey, false),
        AccountMeta::new(*admin_fee_destination_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'swap' instruction.
pub fn swap(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    swap_source_pubkey: &Pubkey,
    swap_destination_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    reward_token_pubkey: &Pubkey,
    reward_mint_pubkey: &Pubkey,
    admin_fee_destination_pubkey: &Pubkey,
    pyth_key: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    swap_direction: u8,
    curve_mode: u8,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Swap(SwapData {
        amount_in,
        minimum_amount_out,
        swap_direction,
        curve_mode,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*swap_source_pubkey, false),
        AccountMeta::new(*swap_destination_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*reward_token_pubkey, false),
        AccountMeta::new(*reward_mint_pubkey, false),
        AccountMeta::new(*admin_fee_destination_pubkey, false),
        AccountMeta::new(*pyth_key, false),
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw_one' instruction.
pub fn withdraw_one(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    swap_base_token_pubkey: &Pubkey,
    swap_quote_token_pubkey: &Pubkey,
    base_destination_pubkey: &Pubkey,
    admin_fee_destination_pubkey: &Pubkey,
    pool_token_amount: u64,
    minimum_token_amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::WithdrawOne(WithdrawOneData {
        pool_token_amount,
        minimum_token_amount,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*swap_base_token_pubkey, false),
        AccountMeta::new(*swap_quote_token_pubkey, false),
        AccountMeta::new(*base_destination_pubkey, false),
        AccountMeta::new(*admin_fee_destination_pubkey, false),
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Instructions supported by the Farming feature.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum FarmingInstruction {
    ///   Deposit some tokens into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   1. `[]` Farm
    ///   2. `[]` $authority,
    ///   3. `[writable]` user farming account,
    ///   6. `[]` owner.
    ///   8. `[]` Token program id
    EnableUser(),
    ///   Deposit some tokens into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` token_a $authority can transfer amount,
    ///   3. `[writable]` token_b $authority can transfer amount,
    ///   4. `[writable]` token_a Base Account to deposit into.
    ///   5. `[writable]` token_b Base Account to deposit into.
    ///   6. `[writable]` Pool MINT account, $authority is the owner.
    ///   7. `[writable]` Pool Account to deposit the generated tokens, user is the owner.
    ///   8. `[]` Token program id
    ///   9. `[]` Clock sysvar
    Deposit(FarmingDepositData),

    ///   Withdraw tokens from the pool at the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` Pool mint account, $authority is the owner
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    ///   4. `[writable]` token_a Swap Account to withdraw FROM.
    ///   5. `[writable]` token_b Swap Account to withdraw FROM.
    ///   6. `[writable]` token_a user Account to credit.
    ///   7. `[writable]` token_b user Account to credit.
    ///   8. `[writable]` admin_fee_a admin fee Account for token_a.
    ///   9. `[writable]` admin_fee_b admin fee Account for token_b.
    ///   10. `[]` Token program id
    Withdraw(FarmingWithdrawData),

    ///   Withdraw tokens from the pool at the current ratio forceful.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` Pool mint account, $authority is the owner
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    ///   4. `[writable]` token_a Swap Account to withdraw FROM.
    ///   5. `[writable]` token_b Swap Account to withdraw FROM.
    ///   6. `[writable]` token_a user Account to credit.
    ///   7. `[writable]` token_b user Account to credit.
    ///   8. `[writable]` admin_fee_a admin fee Account for token_a.
    ///   9. `[writable]` admin_fee_b admin fee Account for token_b.
    ///   10. `[]` Token program id
    EmergencyWithdraw(),

    ///   Withdraw tokens from the pool at the current ratio forceful.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` Pool mint account, $authority is the owner
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    ///   4. `[writable]` token_a Swap Account to withdraw FROM.
    ///   5. `[writable]` token_b Swap Account to withdraw FROM.
    ///   6. `[writable]` token_a user Account to credit.
    ///   7. `[writable]` token_b user Account to credit.
    ///   8. `[writable]` admin_fee_a admin fee Account for token_a.
    ///   9. `[writable]` admin_fee_b admin fee Account for token_b.
    ///   10. `[]` Token program id
    PrintPendingDeltafi(),
}

impl FarmingInstruction {
    /// Unpacks a byte buffer into a [FarmingInstruction](enum.FarmingInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            0x1e => Self::EnableUser(),
            0x1f => {
                let (pool_token_amount, _rest) = unpack_u64(rest)?;
                // let (_min_mint_amount, _rest) = unpack_u64(rest)?;
                Self::Deposit(FarmingDepositData {
                    pool_token_amount,
                    // min_mint_amount,
                })
            }
            0x20 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (min_pool_token_amount, _rest) = unpack_u64(rest)?;
                Self::Withdraw(FarmingWithdrawData {
                    pool_token_amount,
                    min_pool_token_amount,
                })
            }
            0x21 => Self::EmergencyWithdraw(),
            0x22 => Self::PrintPendingDeltafi(),
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [FarmingInstruction](enum.FarmingInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::EnableUser() => {
                buf.push(0x1e);
            }
            Self::Deposit(FarmingDepositData {
                pool_token_amount,
                // min_mint_amount,
            }) => {
                buf.push(0x1f);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                // buf.extend_from_slice(&min_mint_amount.to_le_bytes());
            }
            Self::Withdraw(FarmingWithdrawData {
                pool_token_amount,
                min_pool_token_amount,
            }) => {
                buf.push(0x20);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&min_pool_token_amount.to_le_bytes());
            }
            Self::EmergencyWithdraw() => {
                buf.push(0x21);
            }
            Self::PrintPendingDeltafi() => {
                buf.push(0x22);
            }
        }
        buf
    }
}

/// Creates a 'farm_enable_user' instruction.
pub fn farm_enable_user(
    program_id: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    user_farming_pubkey: &Pubkey,
    // owner: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = FarmingInstruction::EnableUser().pack();

    let accounts = vec![
        AccountMeta::new(*farm_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*user_farming_pubkey, false),
        // AccountMeta::new(*owner, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'farm_deposit' instruction.
pub fn farm_deposit(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    farm_base_pubkey: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_fee_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    user_farming_pubkey: &Pubkey,
    pool_token_pubkey: &Pubkey,
    deltafi_mint_pubkey: &Pubkey,
    dest_pubkey: &Pubkey,
    pool_token_amount: u64,
    _min_mint_amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = FarmingInstruction::Deposit(FarmingDepositData {
        pool_token_amount,
        // min_mint_amount,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*farm_base_pubkey, false),
        AccountMeta::new(*farm_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_fee_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*user_farming_pubkey, false),
        AccountMeta::new(*pool_token_pubkey, false),
        AccountMeta::new(*deltafi_mint_pubkey, false),
        AccountMeta::new(*dest_pubkey, false),
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'farm_withdraw' instruction.
pub fn farm_withdraw(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    farm_base_pubkey: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_fee_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    user_farming_pubkey: &Pubkey,
    pool_token_pubkey: &Pubkey,
    deltafi_mint_pubkey: &Pubkey,
    dest_pubkey: &Pubkey,
    pool_token_amount: u64,
    min_pool_token_amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = FarmingInstruction::Withdraw(FarmingWithdrawData {
        pool_token_amount,
        min_pool_token_amount,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*farm_base_pubkey, false),
        AccountMeta::new(*farm_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_fee_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*user_farming_pubkey, false),
        AccountMeta::new(*pool_token_pubkey, false),
        AccountMeta::new(*deltafi_mint_pubkey, false),
        AccountMeta::new(*dest_pubkey, false),
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'farm_emergency_withdraw' instruction.
pub fn farm_emergency_withdraw(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    farm_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    user_farming_pubkey: &Pubkey,
    pool_token_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = FarmingInstruction::EmergencyWithdraw().pack();

    let accounts = vec![
        AccountMeta::new(*farm_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*user_farming_pubkey, false),
        AccountMeta::new(*pool_token_pubkey, false),
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'farm_pending_deltafi' instruction.
pub fn farm_pending_deltafi(
    program_id: &Pubkey,
    farm_base_pubkey: &Pubkey,
    farm_pubkey: &Pubkey,
    user_farming_pubkey: &Pubkey,
    pool_token_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = FarmingInstruction::PrintPendingDeltafi().pack();

    let accounts = vec![
        AccountMeta::new(*farm_base_pubkey, false),
        AccountMeta::new(*farm_pubkey, false),
        AccountMeta::new(*user_farming_pubkey, false),
        AccountMeta::new(*pool_token_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

fn unpack_i64(input: &[u8]) -> Result<(i64, &[u8]), ProgramError> {
    if input.len() < 8 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (amount, rest) = input.split_at(8);
    let amount = amount
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(i64::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((amount, rest))
}

fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
    if input.len() < 8 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (amount, rest) = input.split_at(8);
    let amount = amount
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((amount, rest))
}

#[allow(dead_code)]
fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
    if input.is_empty() {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (bytes, rest) = input.split_at(1);
    let value = bytes
        .get(..1)
        .and_then(|slice| slice.try_into().ok())
        .map(u8::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((value, rest))
}

#[allow(dead_code)]
fn unpack_bytes32(input: &[u8]) -> Result<(&[u8; 32], &[u8]), ProgramError> {
    if input.len() < 32 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (bytes, rest) = input.split_at(32);
    Ok((
        bytes
            .try_into()
            .map_err(|_| SwapError::InstructionUnpackError)?,
        rest,
    ))
}

#[allow(dead_code)]
fn unpack_pubkey(input: &[u8]) -> Result<(Pubkey, &[u8]), ProgramError> {
    if input.len() < PUBKEY_BYTES {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (key, rest) = input.split_at(PUBKEY_BYTES);
    let pk = Pubkey::new(key);
    Ok((pk, rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{
        test_utils::{default_i, default_k},
        CURVE_PMM, SWAP_DIRECTION_SELL_BASE, TWAP_OPENED,
    };

    #[test]
    fn test_admin_instruction_packing() {
        let target_amp = 100;
        let stop_ramp_ts = i64::MAX;
        let check = AdminInstruction::RampA(RampAData {
            target_amp,
            stop_ramp_ts,
        });
        let packed = check.pack();
        let mut expect: Vec<u8> = vec![101];
        expect.extend_from_slice(&target_amp.to_le_bytes());
        expect.extend_from_slice(&stop_ramp_ts.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the admin instruction to RampA"
        );

        let check = AdminInstruction::StopRampA;
        let packed = check.pack();
        let expect: Vec<u8> = vec![102];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the admin instruction to StopRampA"
        );

        let check = AdminInstruction::Pause;
        let packed = check.pack();
        let expect: Vec<u8> = vec![103];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the admin instruction to Pause"
        );

        let check = AdminInstruction::Unpause;
        let packed = check.pack();
        let expect: Vec<u8> = vec![104];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the admin instruction to Unpause"
        );

        let check = AdminInstruction::SetFeeAccount;
        let packed = check.pack();
        let expect: Vec<u8> = vec![105];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let check = AdminInstruction::ApplyNewAdmin;
        let packed = check.pack();
        let expect: Vec<u8> = vec![106];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the admin instruction to ApplyNewAdmin"
        );

        let check = AdminInstruction::CommitNewAdmin;
        let packed = check.pack();
        let expect: Vec<u8> = vec![107];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the admin instruction to CommitNewAdmin"
        );

        let new_fees = Fees {
            admin_trade_fee_numerator: 1,
            admin_trade_fee_denominator: 2,
            admin_withdraw_fee_numerator: 3,
            admin_withdraw_fee_denominator: 4,
            trade_fee_numerator: 5,
            trade_fee_denominator: 6,
            withdraw_fee_numerator: 7,
            withdraw_fee_denominator: 8,
        };
        let check = AdminInstruction::SetNewFees(new_fees);
        let packed = check.pack();
        let mut expect: Vec<u8> = vec![108];
        let mut new_fees_slice = [0u8; Fees::LEN];
        new_fees.pack_into_slice(&mut new_fees_slice[..]);
        expect.extend_from_slice(&new_fees_slice);
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let new_rewards = Rewards {
            trade_reward_numerator: 1,
            trade_reward_denominator: 2,
            trade_reward_cap: 100,
        };
        let check = AdminInstruction::SetNewRewards(new_rewards);
        let packed = check.pack();
        let mut expect: Vec<u8> = vec![112];
        let mut new_rewards_slice = [0u8; Rewards::LEN];
        new_rewards.pack_into_slice(&mut new_rewards_slice[..]);
        expect.extend_from_slice(&new_rewards_slice);
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let nonce: u8 = 255;
        let alloc_point = 10;
        let reward_unit = 2;
        let check = AdminInstruction::InitializeFarm(FarmData {
            nonce,
            alloc_point,
            reward_unit,
        });
        let packed = check.pack();
        let mut expect = vec![109, nonce];
        expect.extend_from_slice(&alloc_point.to_le_bytes());
        expect.extend_from_slice(&reward_unit.to_le_bytes());
        assert_eq!(
            packed, expect,
            "test packing and unpacking of the admin instruction to InitializeFarm"
        );
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let nonce: u8 = 255;
        let alloc_point = 10;
        let reward_unit = 2;
        let check = AdminInstruction::SetFarm(FarmData {
            nonce,
            alloc_point,
            reward_unit,
        });
        let packed = check.pack();
        let mut expect = vec![110, nonce];
        expect.extend_from_slice(&alloc_point.to_le_bytes());
        expect.extend_from_slice(&reward_unit.to_le_bytes());
        assert_eq!(
            packed, expect,
            "test packing and unpacking of the admin instruction to SetFarm"
        );
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_swap_instruction_packing() {
        // Initialize instruction packing
        {
            let nonce: u8 = 255;
            let k = default_k().inner();
            let i = default_i().inner();
            let is_open_twap = TWAP_OPENED;
            let curve_mode = CURVE_PMM;

            let check = SwapInstruction::Initialize(InitializeData {
                nonce,
                k,
                i,
                is_open_twap,
                curve_mode,
            });
            let packed = check.pack();
            let mut expect: Vec<u8> = vec![0, nonce];
            expect.extend_from_slice(&k.to_le_bytes());
            expect.extend_from_slice(&i.to_le_bytes());
            expect.extend_from_slice(&is_open_twap.to_le_bytes());
            expect.extend_from_slice(&curve_mode.to_le_bytes());
            assert_eq!(packed, expect);
            let unpacked_result = SwapInstruction::unpack(&expect);
            match unpacked_result {
                Ok(unpacked) => match unpacked {
                    Some(instruction) => {
                        assert_eq!(instruction, check);
                    }
                    None => (),
                },
                Err(_) => {}
            }
        }

        // Swap instruction packing
        {
            let amount_in: u64 = 2;
            let minimum_amount_out: u64 = 10;
            let swap_direction: u8 = SWAP_DIRECTION_SELL_BASE;
            let curve_mode: u8 = CURVE_PMM;
            let check = SwapInstruction::Swap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
                curve_mode,
            });
            let packed = check.pack();
            let mut expect = vec![1];
            expect.extend_from_slice(&amount_in.to_le_bytes());
            expect.extend_from_slice(&minimum_amount_out.to_le_bytes());
            expect.extend_from_slice(&swap_direction.to_le_bytes());
            expect.extend_from_slice(&curve_mode.to_le_bytes());
            assert_eq!(packed, expect);
            let unpacked_result = SwapInstruction::unpack(&expect);
            match unpacked_result {
                Ok(unpacked) => match unpacked {
                    Some(instruction) => {
                        assert_eq!(instruction, check);
                    }
                    None => (),
                },
                Err(_) => {}
            }
        }

        // Deposit instruction packing
        {
            let token_a_amount: u64 = 10;
            let token_b_amount: u64 = 20;
            let min_mint_amount: u64 = 5;
            let curve_mode: u8 = CURVE_PMM;
            let check = SwapInstruction::Deposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
                curve_mode,
            });
            let packed = check.pack();
            let mut expect = vec![2];
            expect.extend_from_slice(&token_a_amount.to_le_bytes());
            expect.extend_from_slice(&token_b_amount.to_le_bytes());
            expect.extend_from_slice(&min_mint_amount.to_le_bytes());
            expect.extend_from_slice(&curve_mode.to_le_bytes());
            assert_eq!(packed, expect);
            let unpacked_result = SwapInstruction::unpack(&expect);
            match unpacked_result {
                Ok(unpacked) => match unpacked {
                    Some(instruction) => {
                        assert_eq!(instruction, check);
                    }
                    None => (),
                },
                Err(_) => {}
            }
        }

        // Withdraw instruction packing
        {
            let pool_token_amount: u64 = 1212438012089;
            let minimum_token_a_amount: u64 = 102198761982612;
            let minimum_token_b_amount: u64 = 2011239855213;
            let check = SwapInstruction::Withdraw(WithdrawData {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            });
            let packed = check.pack();
            let mut expect = vec![3];
            expect.extend_from_slice(&pool_token_amount.to_le_bytes());
            expect.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
            expect.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            assert_eq!(packed, expect);
            let unpacked_result = SwapInstruction::unpack(&expect);
            match unpacked_result {
                Ok(unpacked) => match unpacked {
                    Some(instruction) => {
                        assert_eq!(instruction, check);
                    }
                    None => (),
                },
                Err(_) => {}
            }
        }

        // WithdrawOne instruction packing
        {
            let pool_token_amount: u64 = 1212438012089;
            let minimum_token_amount: u64 = 102198761982612;
            let check = SwapInstruction::WithdrawOne(WithdrawOneData {
                pool_token_amount,
                minimum_token_amount,
            });
            let packed = check.pack();
            let mut expect = vec![4];
            expect.extend_from_slice(&pool_token_amount.to_le_bytes());
            expect.extend_from_slice(&minimum_token_amount.to_le_bytes());
            assert_eq!(packed, expect);
            let unpacked_result = SwapInstruction::unpack(&expect);
            match unpacked_result {
                Ok(unpacked) => match unpacked {
                    Some(instruction) => {
                        assert_eq!(instruction, check);
                    }
                    None => (),
                },
                Err(_) => {}
            }
        }
    }

    #[test]
    fn test_farming_instruction_packing() {
        let pool_token_amount: u64 = 10;
        // let min_mint_amount: u64 = 5;
        let check = FarmingInstruction::Deposit(FarmingDepositData {
            pool_token_amount,
            // min_mint_amount,
        });
        let packed = check.pack();
        let mut expect = vec![0x1f];
        expect.extend_from_slice(&pool_token_amount.to_le_bytes());
        // expect.extend_from_slice(&min_mint_amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = FarmingInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the instruction to deposit into farm"
        );

        let pool_token_amount: u64 = 1212438012089;
        let min_pool_token_amount: u64 = 1021987682612;
        let check = FarmingInstruction::Withdraw(FarmingWithdrawData {
            pool_token_amount,
            min_pool_token_amount,
        });
        let packed = check.pack();
        let mut expect = vec![0x20];
        expect.extend_from_slice(&pool_token_amount.to_le_bytes());
        expect.extend_from_slice(&min_pool_token_amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = FarmingInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the instruction to withdraw from farm"
        );

        let check = FarmingInstruction::EmergencyWithdraw();
        let packed = check.pack();
        let expect = vec![0x21];
        assert_eq!(packed, expect);
        let unpacked = FarmingInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the instruction to withdraw emergency from farm"
        );

        let check = FarmingInstruction::PrintPendingDeltafi();
        let packed = check.pack();
        let expect = vec![0x22];
        assert_eq!(packed, expect);
        let unpacked = FarmingInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked, check,
            "test packing and unpacking of the instruction to print pending deltafi in farm"
        );
    }
}
