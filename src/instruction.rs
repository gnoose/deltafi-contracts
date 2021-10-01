//! Instruction types

#![allow(clippy::too_many_arguments)]

use std::{convert::TryInto, mem::size_of};

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::clock,
};

use crate::{error::SwapError, fees::Fees, rewards::Rewards};

/// SWAP INSTRUNCTION DATA
/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct InitializeData {
    /// Nonce used to create valid program address
    pub nonce: u8,
    /// Amplification coefficient (A)
    pub amp_factor: u64,
    /// Fees
    pub fees: Fees,
    /// Rewards
    pub rewards: Rewards,
    /// Slope variable - real value * 10**6
    pub k: u64,
    /// mid price 0 ~ 10**6
    pub i: u64,
    /// flag to know about twap open
    pub is_open_twap: u64,
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
    pub swap_direction: u64,
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
    pub fn unpack(input: &[u8]) -> Result<Option<Self>, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            0x64 => {
                let (target_amp, rest) = unpack_u64(rest)?;
                let (stop_ramp_ts, _rest) = unpack_i64(rest)?;
                Some(Self::RampA(RampAData {
                    target_amp,
                    stop_ramp_ts,
                }))
            }
            0x65 => Some(Self::StopRampA),
            0x66 => Some(Self::Pause),
            0x67 => Some(Self::Unpause),
            0x68 => Some(Self::SetFeeAccount),
            0x69 => Some(Self::ApplyNewAdmin),
            0x6A => Some(Self::CommitNewAdmin),
            0x6B => {
                let fees = Fees::unpack_unchecked(rest)?;
                Some(Self::SetNewFees(fees))
            }
            0x6C => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (alloc_point, rest) = unpack_u64(rest)?;
                let (reward_unit, _rest) = unpack_u64(rest)?;
                Some(Self::InitializeFarm(FarmData {
                    nonce,
                    alloc_point,
                    reward_unit,
                }))
            }
            0x6D => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (alloc_point, rest) = unpack_u64(rest)?;
                let (reward_unit, _rest) = unpack_u64(rest)?;
                Some(Self::SetFarm(FarmData {
                    nonce,
                    alloc_point,
                    reward_unit,
                }))
            }
            0x6E => Some(Self::ApplyNewAdminForFarm),
            0x6F => {
                let rewards = Rewards::unpack_unchecked(rest)?;
                Some(Self::SetNewRewards(rewards))
            }
            _ => None,
        })
    }

    /// Packs a [AdminInstruction](enum.AdminInstruciton.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::RampA(RampAData {
                target_amp,
                stop_ramp_ts,
            }) => {
                buf.push(0x64);
                buf.extend_from_slice(&target_amp.to_le_bytes());
                buf.extend_from_slice(&stop_ramp_ts.to_le_bytes());
            }
            Self::StopRampA => buf.push(0x65),
            Self::Pause => buf.push(0x66),
            Self::Unpause => buf.push(0x67),
            Self::SetFeeAccount => buf.push(0x68),
            Self::ApplyNewAdmin => buf.push(0x69),
            Self::CommitNewAdmin => buf.push(0x6a),
            Self::SetNewFees(fees) => {
                buf.push(0x6b);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(&fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
            }
            Self::InitializeFarm(FarmData {
                nonce,
                alloc_point,
                reward_unit,
            }) => {
                buf.push(0x6c);
                buf.push(nonce);
                buf.extend_from_slice(&alloc_point.to_le_bytes());
                buf.extend_from_slice(&reward_unit.to_le_bytes());
            }
            Self::SetFarm(FarmData {
                nonce,
                alloc_point,
                reward_unit,
            }) => {
                buf.push(0x6d);
                buf.push(nonce);
                buf.extend_from_slice(&alloc_point.to_le_bytes());
                buf.extend_from_slice(&reward_unit.to_le_bytes());
            }
            Self::ApplyNewAdminForFarm => buf.push(0x6e),
            Self::SetNewRewards(rewards) => {
                buf.push(0x6f);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(&rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
        }
        buf
    }
}

/// Creates a 'ramp_a' instruction
pub fn ramp_a(
    program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
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
        AccountMeta::new(*swap_pubkey, true),
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

/// Creates a 'stop_ramp_a' instruction
pub fn stop_ramp_a(
    program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::StopRampA.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
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

/// Creates a 'pause' instruction
pub fn pause(
    program_id: &Pubkey,
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Pause.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
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
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Unpause.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
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
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::ApplyNewAdmin.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
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
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_admin_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::CommitNewAdmin.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
        AccountMeta::new(*new_admin_pubkey, false),
        AccountMeta::new(clock::id(), false),
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
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_fee_account_pubkey: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetFeeAccount.pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
        AccountMeta::new(*new_fee_account_pubkey, false),
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
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_fees: Fees,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewFees(new_fees).pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
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
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    new_rewards: Rewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewRewards(new_rewards).pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, true),
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

/// Instructions supported by the stableswap program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum StableInstruction {
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
    StableInitialize(InitializeData),

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
    StableSwap(SwapData),

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
    StableDeposit(DepositData),

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
    StableWithdraw(WithdrawData),

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
    StableWithdrawOne(WithdrawOneData),
}

impl StableInstruction {
    /// Unpacks a byte buffer into a [StableInstruction](enum.StableInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Option<Self>, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            10 => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (amp_factor, rest) = unpack_u64(rest)?;
                let (fees, rest) = rest.split_at(Fees::LEN);
                let fees = Fees::unpack_unchecked(fees)?;
                let (rewards, rest) = rest.split_at(Rewards::LEN);
                let rewards = Rewards::unpack_unchecked(rewards)?;
                let (k, rest) = unpack_u64(rest)?;
                let (i, rest) = unpack_u64(rest)?;
                let (is_open_twap, _rest) = unpack_u64(rest)?;
                Some(Self::StableInitialize(InitializeData {
                    nonce,
                    amp_factor,
                    fees,
                    rewards,
                    k,
                    i,
                    is_open_twap,
                }))
            }
            11 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, rest) = unpack_u64(rest)?;
                let (swap_direction, _rest) = unpack_u64(rest)?;
                Some(Self::StableSwap(SwapData {
                    amount_in,
                    minimum_amount_out,
                    swap_direction,
                }))
            }
            12 => {
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (min_mint_amount, _rest) = unpack_u64(rest)?;
                Some(Self::StableDeposit(DepositData {
                    token_a_amount,
                    token_b_amount,
                    min_mint_amount,
                }))
            }
            13 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_b_amount, _rest) = unpack_u64(rest)?;
                Some(Self::StableWithdraw(WithdrawData {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                }))
            }
            14 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_amount, _rest) = unpack_u64(rest)?;
                Some(Self::StableWithdrawOne(WithdrawOneData {
                    pool_token_amount,
                    minimum_token_amount,
                }))
            }
            _ => None,
        })
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::StableInitialize(InitializeData {
                nonce,
                amp_factor,
                fees,
                rewards,
                k,
                i,
                is_open_twap,
            }) => {
                buf.push(0);
                buf.push(nonce);
                buf.extend_from_slice(&amp_factor.to_le_bytes());
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(&fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(&rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
                buf.extend_from_slice(&k.to_le_bytes());
                buf.extend_from_slice(&i.to_le_bytes());
                buf.extend_from_slice(&is_open_twap.to_le_bytes());
            }
            Self::StableSwap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
            }) => {
                buf.push(1);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
                buf.extend_from_slice(&swap_direction.to_le_bytes());
            }
            Self::StableDeposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
            }) => {
                buf.push(2);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
                buf.extend_from_slice(&min_mint_amount.to_le_bytes());
            }
            Self::StableWithdraw(WithdrawData {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                buf.push(3);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            }
            Self::StableWithdrawOne(WithdrawOneData {
                pool_token_amount,
                minimum_token_amount,
            }) => {
                buf.push(4);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_amount.to_le_bytes());
            }
        }
        buf
    }
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
}

impl SwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Option<Self>, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            0x0 => {
                let (&nonce, rest) = rest.split_first().ok_or(SwapError::InvalidInstruction)?;
                let (amp_factor, rest) = unpack_u64(rest)?;
                let (fees, rest) = rest.split_at(Fees::LEN);
                let fees = Fees::unpack_unchecked(fees)?;
                let (rewards, rest) = rest.split_at(Rewards::LEN);
                let rewards = Rewards::unpack_unchecked(rewards)?;
                let (k, rest) = unpack_u64(rest)?;
                let (i, rest) = unpack_u64(rest)?;
                let (is_open_twap, _rest) = unpack_u64(rest)?;
                Some(Self::Initialize(InitializeData {
                    nonce,
                    amp_factor,
                    fees,
                    rewards,
                    k,
                    i,
                    is_open_twap,
                }))
            }
            0x1 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, rest) = unpack_u64(rest)?;
                let (swap_direction, _rest) = unpack_u64(rest)?;
                Some(Self::Swap(SwapData {
                    amount_in,
                    minimum_amount_out,
                    swap_direction,
                }))
            }
            0x2 => {
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (min_mint_amount, _rest) = unpack_u64(rest)?;
                Some(Self::Deposit(DepositData {
                    token_a_amount,
                    token_b_amount,
                    min_mint_amount,
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
            _ => None,
        })
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(InitializeData {
                nonce,
                amp_factor,
                fees,
                rewards,
                k,
                i,
                is_open_twap,
            }) => {
                buf.push(0x0);
                buf.push(nonce);
                buf.extend_from_slice(&amp_factor.to_le_bytes());
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(&fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(&rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
                buf.extend_from_slice(&k.to_le_bytes());
                buf.extend_from_slice(&i.to_le_bytes());
                buf.extend_from_slice(&is_open_twap.to_le_bytes());
            }
            Self::Swap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
            }) => {
                buf.push(0x1);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
                buf.extend_from_slice(&swap_direction.to_le_bytes());
            }
            Self::Deposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
            }) => {
                buf.push(0x2);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
                buf.extend_from_slice(&min_mint_amount.to_le_bytes());
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
        }
        buf
    }
}

/// Creates an 'stable_initialize' instruction.
pub fn stable_initialize(
    program_id: &Pubkey,
    pool_token_program_id: &Pubkey, // Token program used for the pool token
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    admin_fee_a_pubkey: &Pubkey,
    admin_fee_b_pubkey: &Pubkey,
    token_a_mint_pubkey: &Pubkey,
    token_a_pubkey: &Pubkey,
    token_b_mint_pubkey: &Pubkey,
    token_b_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    destination_pubkey: &Pubkey, // Desintation to mint pool tokens for bootstrapper
    deltafi_mint_pubkey: &Pubkey,
    deltafi_token_pubkey: &Pubkey,
    nonce: u8,
    amp_factor: u64,
    fees: Fees,
    rewards: Rewards,
    k: u64,
    i: u64,
    is_open_twap: u64,
) -> Result<Instruction, ProgramError> {
    let data = StableInstruction::StableInitialize(InitializeData {
        nonce,
        amp_factor,
        fees,
        rewards,
        k,
        i,
        is_open_twap,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, false),
        AccountMeta::new(*admin_fee_a_pubkey, false),
        AccountMeta::new(*admin_fee_b_pubkey, false),
        AccountMeta::new(*token_a_mint_pubkey, false),
        AccountMeta::new(*token_a_pubkey, false),
        AccountMeta::new(*token_b_mint_pubkey, false),
        AccountMeta::new(*token_b_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*deltafi_mint_pubkey, false),
        AccountMeta::new(*deltafi_token_pubkey, false),
        AccountMeta::new(*pool_token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    pool_token_program_id: &Pubkey, // Token program used for the pool token
    swap_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    admin_pubkey: &Pubkey,
    admin_fee_a_pubkey: &Pubkey,
    admin_fee_b_pubkey: &Pubkey,
    token_a_mint_pubkey: &Pubkey,
    token_a_pubkey: &Pubkey,
    token_b_mint_pubkey: &Pubkey,
    token_b_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    destination_pubkey: &Pubkey, // Desintation to mint pool tokens for bootstrapper
    deltafi_mint_pubkey: &Pubkey,
    deltafi_token_pubkey: &Pubkey,
    nonce: u8,
    amp_factor: u64,
    fees: Fees,
    rewards: Rewards,
    k: u64,
    i: u64,
    is_open_twap: u64,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Initialize(InitializeData {
        nonce,
        amp_factor,
        fees,
        rewards,
        k,
        i,
        is_open_twap,
    })
    .pack();

    let accounts = vec![
        AccountMeta::new(*swap_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*admin_pubkey, false),
        AccountMeta::new(*admin_fee_a_pubkey, false),
        AccountMeta::new(*admin_fee_b_pubkey, false),
        AccountMeta::new(*token_a_mint_pubkey, false),
        AccountMeta::new(*token_a_pubkey, false),
        AccountMeta::new(*token_b_mint_pubkey, false),
        AccountMeta::new(*token_b_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*deltafi_mint_pubkey, false),
        AccountMeta::new(*deltafi_token_pubkey, false),
        AccountMeta::new(*pool_token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'stable_deposit' instruction.
pub fn stable_deposit(
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
    token_a_amount: u64,
    token_b_amount: u64,
    min_mint_amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = StableInstruction::StableDeposit(DepositData {
        token_a_amount,
        token_b_amount,
        min_mint_amount,
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
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
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
    token_a_amount: u64,
    token_b_amount: u64,
    min_mint_amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Deposit(DepositData {
        token_a_amount,
        token_b_amount,
        min_mint_amount,
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

/// Creates a 'swap' instruction.
pub fn stable_swap(
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
    amount_in: u64,
    minimum_amount_out: u64,
    swap_direction: u64,
) -> Result<Instruction, ProgramError> {
    let data = StableInstruction::StableSwap(SwapData {
        amount_in,
        minimum_amount_out,
        swap_direction,
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
        AccountMeta::new(*token_program_id, false),
        AccountMeta::new(clock::id(), false),
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
    amount_in: u64,
    minimum_amount_out: u64,
    swap_direction: u64,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Swap(SwapData {
        amount_in,
        minimum_amount_out,
        swap_direction,
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
    if input.len() >= 8 {
        let (amount, rest) = input.split_at(8);
        let amount = amount
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(i64::from_le_bytes)
            .ok_or(SwapError::InvalidInstruction)?;
        Ok((amount, rest))
    } else {
        Err(SwapError::InvalidInstruction.into())
    }
}

fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
    if input.len() >= 8 {
        let (amount, rest) = input.split_at(8);
        let amount = amount
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(SwapError::InvalidInstruction)?;
        Ok((amount, rest))
    } else {
        Err(SwapError::InvalidInstruction.into())
    }
}

/// Unpacks a reference from a bytes buffer.
/// TODO actually pack / unpack instead of relying on normal memory layout.
pub fn unpack<T>(input: &[u8]) -> Result<&T, ProgramError> {
    if input.len() < size_of::<u8>() + size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }
    #[allow(clippy::cast_ptr_alignment)]
    let val: &T = unsafe { &*(&input[1] as *const u8 as *const T) };
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{
        test_utils::{default_i, default_k},
        SWAP_DIRECTION_SELL_BASE, TWAP_OPENED,
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
        let mut expect: Vec<u8> = vec![0x64];
        expect.extend_from_slice(&target_amp.to_le_bytes());
        expect.extend_from_slice(&stop_ramp_ts.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked,
            Some(check),
            "test packing and unpacking of the admin instruction to RampA"
        );

        let check = AdminInstruction::StopRampA;
        let packed = check.pack();
        let expect: Vec<u8> = vec![0x65];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked,
            Some(check),
            "test packing and unpacking of the admin instruction to StopRampA"
        );

        let check = AdminInstruction::Pause;
        let packed = check.pack();
        let expect: Vec<u8> = vec![0x66];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked,
            Some(check),
            "test packing and unpacking of the admin instruction to Pause"
        );

        let check = AdminInstruction::Unpause;
        let packed = check.pack();
        let expect: Vec<u8> = vec![0x67];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked,
            Some(check),
            "test packing and unpacking of the admin instruction to Unpause"
        );

        let check = AdminInstruction::SetFeeAccount;
        let packed = check.pack();
        let expect: Vec<u8> = vec![0x68];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, Some(check));

        let check = AdminInstruction::ApplyNewAdmin;
        let packed = check.pack();
        let expect: Vec<u8> = vec![0x69];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked,
            Some(check),
            "test packing and unpacking of the admin instruction to ApplyNewAdmin"
        );

        let check = AdminInstruction::CommitNewAdmin;
        let packed = check.pack();
        let expect: Vec<u8> = vec![0x6A];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(
            unpacked,
            Some(check),
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
        let mut expect: Vec<u8> = vec![0x6B];
        let mut new_fees_slice = [0u8; Fees::LEN];
        new_fees.pack_into_slice(&mut new_fees_slice[..]);
        expect.extend_from_slice(&new_fees_slice);
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, Some(check));

        let new_rewards = Rewards {
            trade_reward_numerator: 1,
            trade_reward_denominator: 2,
            trade_reward_cap: 100,
        };
        let check = AdminInstruction::SetNewRewards(new_rewards);
        let packed = check.pack();
        let mut expect: Vec<u8> = vec![0x6F];
        let mut new_rewards_slice = [0u8; Rewards::LEN];
        new_rewards.pack_into_slice(&mut new_rewards_slice[..]);
        expect.extend_from_slice(&new_rewards_slice);
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, Some(check));

        let nonce: u8 = 255;
        let alloc_point = 10;
        let reward_unit = 2;
        let check = AdminInstruction::InitializeFarm(FarmData {
            nonce,
            alloc_point,
            reward_unit,
        });
        let packed = check.pack();
        let mut expect = vec![0x6C, nonce];
        expect.extend_from_slice(&alloc_point.to_le_bytes());
        expect.extend_from_slice(&reward_unit.to_le_bytes());
        assert_eq!(
            packed, expect,
            "test packing and unpacking of the admin instruction to InitializeFarm"
        );
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, Some(check));

        let nonce: u8 = 255;
        let alloc_point = 10;
        let reward_unit = 2;
        let check = AdminInstruction::SetFarm(FarmData {
            nonce,
            alloc_point,
            reward_unit,
        });
        let packed = check.pack();
        let mut expect = vec![0x6D, nonce];
        expect.extend_from_slice(&alloc_point.to_le_bytes());
        expect.extend_from_slice(&reward_unit.to_le_bytes());
        assert_eq!(
            packed, expect,
            "test packing and unpacking of the admin instruction to SetFarm"
        );
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, Some(check));
    }

    #[test]
    fn test_swap_instruction_packing() {
        // Initialize instruction packing
        {
            let nonce: u8 = 255;
            let amp_factor: u64 = 0;
            let fees = Fees {
                admin_trade_fee_numerator: 1,
                admin_trade_fee_denominator: 2,
                admin_withdraw_fee_numerator: 3,
                admin_withdraw_fee_denominator: 4,
                trade_fee_numerator: 5,
                trade_fee_denominator: 6,
                withdraw_fee_numerator: 7,
                withdraw_fee_denominator: 8,
            };
            let rewards = Rewards {
                trade_reward_numerator: 1,
                trade_reward_denominator: 2,
                trade_reward_cap: 100,
            };
            let k = default_k().inner_u64().unwrap();
            let i = default_i().inner_u64().unwrap();
            let is_open_twap = TWAP_OPENED;

            let check = SwapInstruction::Initialize(InitializeData {
                nonce,
                amp_factor,
                fees,
                rewards,
                k,
                i,
                is_open_twap,
            });
            let packed = check.pack();
            let mut expect: Vec<u8> = vec![0, nonce];
            expect.extend_from_slice(&amp_factor.to_le_bytes());
            let mut fees_slice = [0u8; Fees::LEN];
            fees.pack_into_slice(&mut fees_slice[..]);
            expect.extend_from_slice(&fees_slice);
            let mut rewards_slice = [0u8; Rewards::LEN];
            rewards.pack_into_slice(&mut rewards_slice);
            expect.extend_from_slice(&rewards_slice);
            expect.extend_from_slice(&k.to_le_bytes());
            expect.extend_from_slice(&i.to_le_bytes());
            expect.extend_from_slice(&is_open_twap.to_le_bytes());
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
            let swap_direction: u64 = SWAP_DIRECTION_SELL_BASE;
            let check = SwapInstruction::Swap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
            });
            let packed = check.pack();
            let mut expect = vec![1];
            expect.extend_from_slice(&amount_in.to_le_bytes());
            expect.extend_from_slice(&minimum_amount_out.to_le_bytes());
            expect.extend_from_slice(&swap_direction.to_le_bytes());
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
            let check = SwapInstruction::Deposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
            });
            let packed = check.pack();
            let mut expect = vec![2];
            expect.extend_from_slice(&token_a_amount.to_le_bytes());
            expect.extend_from_slice(&token_b_amount.to_le_bytes());
            expect.extend_from_slice(&min_mint_amount.to_le_bytes());
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
