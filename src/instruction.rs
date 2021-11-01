//! Instruction types

#![allow(clippy::too_many_arguments)]

use std::{convert::TryInto, mem::size_of};

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::{Pubkey, PUBKEY_BYTES},
    sysvar::{clock, rent},
};

use crate::{
    error::SwapError,
    state::{Fees, Rewards},
};

/// Instruction Type
#[repr(C)]
pub enum InstructionType {
    /// Admin
    Admin,
    /// Swap
    Swap,
}

impl InstructionType {
    #[doc(hidden)]
    pub fn check(input: &[u8]) -> Option<Self> {
        let (&tag, _rest) = input.split_first()?;
        match tag {
            100..=106 => Some(Self::Admin),
            0..=7 => Some(Self::Swap),
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
    /// Slope variable - real value * 10**18, 0 <= slope <= 1
    pub slope: u64,
    /// mid price
    pub mid_price: u128,
    /// flag to know about twap open
    pub is_open_twap: bool,
}

/// Swap direction
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapDirection {
    /// sell base
    SellBase,
    /// sell quote
    SellQuote,
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
    pub swap_direction: SwapDirection,
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

/// ADMIN INSTRUCTION PARAMS
/// Admin initialize config data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct AdminInitializeData {
    /// Default fees
    pub fees: Fees,
    /// Default rewards
    pub rewards: Rewards,
}

/// Set new admin key
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct CommitNewAdmin {
    /// The new admin
    pub new_admin_key: Pubkey,
}

/// Admin only instructions.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum AdminInstruction {
    /// Admin initialization instruction
    Initialize(AdminInitializeData),
    /// TODO: Docs
    Pause,
    /// TODO: Docs
    Unpause,
    /// TODO: Docs
    SetFeeAccount,
    /// TODO: Docs
    CommitNewAdmin(CommitNewAdmin),
    /// TODO: Docs
    SetNewFees(Fees),
    /// TODO: Docs
    SetNewRewards(Rewards),
}

impl AdminInstruction {
    /// Unpacks a byte buffer into a [AdminInstruction](enum.AdminInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(SwapError::InstructionUnpackError)?;
        Ok(match tag {
            100 => {
                let (fees, rest) = rest.split_at(Fees::LEN);
                let fees = Fees::unpack_unchecked(fees)?;
                let (rewards, _rest) = rest.split_at(Rewards::LEN);
                let rewards = Rewards::unpack_unchecked(rewards)?;
                Self::Initialize(AdminInitializeData { fees, rewards })
            }
            101 => Self::Pause,
            102 => Self::Unpause,
            103 => Self::SetFeeAccount,
            104 => {
                let (new_admin_key, _) = unpack_pubkey(rest)?;
                Self::CommitNewAdmin(CommitNewAdmin { new_admin_key })
            }
            105 => {
                let fees = Fees::unpack_unchecked(rest)?;
                Self::SetNewFees(fees)
            }
            106 => {
                let rewards = Rewards::unpack_unchecked(rest)?;
                Self::SetNewRewards(rewards)
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [AdminInstruction](enum.AdminInstruciton.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match &*self {
            Self::Initialize(AdminInitializeData { fees, rewards }) => {
                buf.push(100);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
            Self::Pause => buf.push(101),
            Self::Unpause => buf.push(102),
            Self::SetFeeAccount => buf.push(103),
            Self::CommitNewAdmin(CommitNewAdmin { new_admin_key }) => {
                buf.push(104);
                buf.extend_from_slice(new_admin_key.as_ref());
            }
            Self::SetNewFees(fees) => {
                buf.push(105);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
            }
            Self::SetNewRewards(rewards) => {
                buf.push(106);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction
pub fn initialize_config(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    deltafi_mint_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    fees: Fees,
    rewards: Rewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Initialize(AdminInitializeData { fees, rewards }).pack();

    let accounts = vec![
        AccountMeta::new(config_pubkey, false),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(deltafi_mint_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(rent::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'pause' instruction
pub fn pause(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Pause.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'unpause' instruction
pub fn unpause(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Unpause.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_fee_account' instruction
pub fn set_fee_account(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    new_fee_account_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetFeeAccount.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(new_fee_account_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'commit_new_admin' instruction
pub fn commit_new_admin(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    deltafi_mint_pubkey: Pubkey,
    new_admin_key: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::CommitNewAdmin(CommitNewAdmin { new_admin_key }).pack();

    let accounts = vec![
        AccountMeta::new(config_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new(deltafi_mint_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_new_fees' instruction
pub fn set_new_fees(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    new_fees: Fees,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewFees(new_fees).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_rewards' instruction.
pub fn set_new_rewards(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    new_rewards: Rewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewRewards(new_rewards).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Instructions supported by the pool SwapInfo program.
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

    // ///   Withdraw one token from the pool at the current ratio.
    // ///
    // ///   0. `[]` Token-swap
    // ///   1. `[]` $authority
    // ///   2. `[writable]` Pool mint account, $authority is the owner
    // ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    // ///   4. `[writable]` token_(A|B) BASE token Swap Account to withdraw FROM.
    // ///   5. `[writable]` token_(A|B) QUOTE token Swap Account to exchange to base token.
    // ///   6. `[writable]` token_(A|B) BASE token user Account to credit.
    // ///   7. `[writable]` token_(A|B) admin fee Account. Must have same mint as BASE token.
    // ///   8. `[]` Token program id
    // ///   9. `[]` Clock sysvar
    // WithdrawOne(WithdrawOneData),

    // ///   Calc the receive amount in the pool.
    // ///
    // ///   0. `[]` Token-swap
    // ///   1. `[]` $authority
    // ///   2. `[writable]` token_(A|B) SOURCE Account, amount is transferable by $authority,
    // ///   3. `[writable]` token_(A|B) Base Account to swap INTO.  Must be the SOURCE token.
    // ///   4. `[writable]` token_(A|B) Base Account to swap FROM.  Must be the DESTINATION token.
    // ///   5. `[writable]` token_(A|B) DESTINATION Account assigned to USER as the owner.
    // ///   6. `[writable]` token_(A|B) admin fee Account. Must have same mint as DESTINATION token.
    // ///   7. `[]` Token program id
    // ///   8. `[]` Clock sysvar
    // CalcReceiveAmount(SwapData),
    /// Initialize liquidity provider account
    ///
    ///   0. `[]` Token-swap
    ///   1. `[writable]` liquidity provider info
    ///   2. `[signer]` liquidity provider owner
    ///   3. `[]` Token program id
    ///   4. `[]` Clock sysvar
    InitializeLiquidityProvider,

    /// Claim deltafi reward of liquidity provider
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` $authority
    ///   2. `[writable]` Liquidity provider info
    ///   3. `[signer]` Liquidity provider owner
    ///   4. `[writable]` Rewards receiver
    ///   5. `[writable]` Rewards mint deltafi
    ///   6. `[]` Token program id
    ClaimLiquidityRewards,

    /// Refresh liquidity obligation
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` Clock sysvar
    ///   .. `[]` Liquidity provider accounts - refreshed, all, in order.
    RefreshLiquidityObligation,
}

impl SwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(SwapError::InstructionUnpackError)?;
        Ok(match tag {
            0x0 => {
                let (&nonce, rest) = rest
                    .split_first()
                    .ok_or(SwapError::InstructionUnpackError)?;
                let (slope, rest) = unpack_u64(rest)?;
                let (mid_price, rest) = unpack_u128(rest)?;
                let (is_open_twap, _) = unpack_bool(rest)?;
                Self::Initialize(InitializeData {
                    nonce,
                    slope,
                    mid_price,
                    is_open_twap,
                })
            }
            0x1 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, rest) = unpack_u64(rest)?;
                let (swap_direction, _) = unpack_swap_direction(rest)?;
                Self::Swap(SwapData {
                    amount_in,
                    minimum_amount_out,
                    swap_direction,
                })
            }
            0x2 => {
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (min_mint_amount, _) = unpack_u64(rest)?;
                Self::Deposit(DepositData {
                    token_a_amount,
                    token_b_amount,
                    min_mint_amount,
                })
            }
            0x3 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_b_amount, _) = unpack_u64(rest)?;
                Self::Withdraw(WithdrawData {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                })
            }
            0x4 => Self::InitializeLiquidityProvider,
            0x5 => Self::ClaimLiquidityRewards,
            0x6 => Self::RefreshLiquidityObligation,
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(InitializeData {
                nonce,
                slope,
                mid_price,
                is_open_twap,
            }) => {
                buf.push(0x0);
                buf.push(nonce);
                buf.extend_from_slice(&slope.to_le_bytes());
                buf.extend_from_slice(&mid_price.to_le_bytes());
                buf.extend_from_slice(&(is_open_twap as u8).to_le_bytes());
            }
            Self::Swap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
            }) => {
                buf.push(0x1);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
                buf.extend_from_slice(&(swap_direction as u8).to_le_bytes());
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
            Self::InitializeLiquidityProvider => {
                buf.push(0x4);
            }
            Self::ClaimLiquidityRewards => {
                buf.push(0x5);
            }
            Self::RefreshLiquidityObligation => {
                buf.push(0x6);
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    admin_fee_a_pubkey: Pubkey,
    admin_fee_b_pubkey: Pubkey,
    token_a_pubkey: Pubkey,
    token_b_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    init_data: InitializeData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Initialize(init_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(admin_fee_a_pubkey, false),
        AccountMeta::new_readonly(admin_fee_b_pubkey, false),
        AccountMeta::new_readonly(token_a_pubkey, false),
        AccountMeta::new_readonly(token_b_pubkey, false),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'swap' instruction.
pub fn swap(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    swap_authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_source_pubkey: Pubkey,
    swap_destination_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    reward_token_pubkey: Pubkey,
    reward_mint_pubkey: Pubkey,
    admin_fee_destination_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    swap_data: SwapData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Swap(swap_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(swap_authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_source_pubkey, false),
        AccountMeta::new(swap_destination_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new(reward_token_pubkey, false),
        AccountMeta::new(reward_mint_pubkey, false),
        AccountMeta::new(admin_fee_destination_pubkey, false),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'deposit' instruction.
pub fn deposit(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    deposit_token_a_pubkey: Pubkey,
    deposit_token_b_pubkey: Pubkey,
    swap_token_a_pubkey: Pubkey,
    swap_token_b_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    liquidity_provider_pubkey: Pubkey,
    liquidity_owner_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    deposit_data: DepositData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Deposit(deposit_data).pack();

    let accounts = vec![
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(deposit_token_a_pubkey, false),
        AccountMeta::new(deposit_token_b_pubkey, false),
        AccountMeta::new(swap_token_a_pubkey, false),
        AccountMeta::new(swap_token_b_pubkey, false),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new(liquidity_provider_pubkey, false),
        AccountMeta::new_readonly(liquidity_owner_pubkey, true),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw' instruction.
pub fn withdraw(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_token_a_pubkey: Pubkey,
    swap_token_b_pubkey: Pubkey,
    destination_token_a_pubkey: Pubkey,
    destination_token_b_pubkey: Pubkey,
    admin_fee_a_pubkey: Pubkey,
    admin_fee_b_pubkey: Pubkey,
    liquidity_provider_pubkey: Pubkey,
    liquidity_owner_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    withdraw_data: WithdrawData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Withdraw(withdraw_data).pack();

    let accounts = vec![
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_token_a_pubkey, false),
        AccountMeta::new(swap_token_b_pubkey, false),
        AccountMeta::new(destination_token_a_pubkey, false),
        AccountMeta::new(destination_token_b_pubkey, false),
        AccountMeta::new(admin_fee_a_pubkey, false),
        AccountMeta::new(admin_fee_b_pubkey, false),
        AccountMeta::new(liquidity_provider_pubkey, false),
        AccountMeta::new_readonly(liquidity_owner_pubkey, true),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates `InitializeLiquidityProvider` instruction
pub fn init_liquidity_provider(
    program_id: Pubkey,
    liquidity_provider_pubkey: Pubkey,
    liquidity_owner_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::InitializeLiquidityProvider.pack();

    let accounts = vec![
        AccountMeta::new(liquidity_provider_pubkey, false),
        AccountMeta::new_readonly(liquidity_owner_pubkey, true),
        AccountMeta::new_readonly(rent::id(), false),
    ];

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

/// Creates `ClaimLiquidityRewards` instruction
pub fn claim_liquidity_rewards(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    market_authority_info: Pubkey,
    liquidity_provider_pubkey: Pubkey,
    liquidity_owner_pubkey: Pubkey,
    claim_destination_pubkey: Pubkey,
    claim_mint_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::ClaimLiquidityRewards.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new_readonly(swap_pubkey, false),
        AccountMeta::new_readonly(market_authority_info, false),
        AccountMeta::new(liquidity_provider_pubkey, false),
        AccountMeta::new_readonly(liquidity_owner_pubkey, true),
        AccountMeta::new(claim_destination_pubkey, false),
        AccountMeta::new(claim_mint_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

/// Creates `RefreshLiquidityObligation` instruction
pub fn refresh_liquidity_obligation(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    liquidity_provider_pubkeys: Vec<Pubkey>,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::RefreshLiquidityObligation.pack();

    let mut accounts = vec![
        AccountMeta::new_readonly(swap_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
    ];
    accounts.extend(
        liquidity_provider_pubkeys
            .into_iter()
            .map(|pubkey| AccountMeta::new(pubkey, false)),
    );

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
    if input.len() < 16 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (amount, rest) = input.split_at(16);
    let amount = amount
        .get(..16)
        .and_then(|slice| slice.try_into().ok())
        .map(u128::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((amount, rest))
}

#[allow(dead_code)]
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

fn unpack_bool(input: &[u8]) -> Result<(bool, &[u8]), ProgramError> {
    let (value, rest) = unpack_u8(input)?;
    let value = match u8::from_le(value) {
        0 => false,
        1 => true,
        _ => return Err(SwapError::InstructionUnpackError.into()),
    };
    Ok((value, rest))
}

fn unpack_swap_direction(input: &[u8]) -> Result<(SwapDirection, &[u8]), ProgramError> {
    let (value, rest) = unpack_u8(input)?;
    let value = match u8::from_le(value) {
        0 => SwapDirection::SellBase,
        1 => SwapDirection::SellQuote,
        _ => return Err(SwapError::InstructionUnpackError.into()),
    };
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
    use crate::{
        curve::{default_market_price, default_slope},
        state::{DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS},
    };

    #[test]
    fn test_pack_admin_init_config() {
        let fees = DEFAULT_TEST_FEES;
        let rewards = DEFAULT_TEST_REWARDS;
        let check = AdminInstruction::Initialize(AdminInitializeData {
            fees: fees.clone(),
            rewards: rewards.clone(),
        });
        let packed = check.pack();
        let mut expect = vec![100];
        expect.extend_from_slice(&fees.admin_trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());
        expect.extend_from_slice(&rewards.liquidity_reward_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards.liquidity_reward_denominator.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_set_new_fees() {
        let fees = DEFAULT_TEST_FEES;
        let check = AdminInstruction::SetNewFees(fees.clone());
        let packed = check.pack();
        let mut expect = vec![105];
        expect.extend_from_slice(&fees.admin_trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_denominator.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_set_new_rewards() {
        let rewards = DEFAULT_TEST_REWARDS;
        let check = AdminInstruction::SetNewRewards(rewards.clone());
        let packed = check.pack();
        let mut expect = vec![106];
        expect.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());
        expect.extend_from_slice(&rewards.liquidity_reward_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards.liquidity_reward_denominator.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_swap_initialization() {
        let nonce: u8 = 255;
        let slope: u64 = default_slope().to_scaled_val().unwrap().try_into().unwrap();
        let mid_price = default_market_price().to_scaled_val().unwrap();
        let is_open_twap = true;
        let check = SwapInstruction::Initialize(InitializeData {
            nonce,
            slope,
            mid_price,
            is_open_twap,
        });
        let packed = check.pack();
        let mut expect = vec![0];
        expect.extend_from_slice(&nonce.to_le_bytes());
        expect.extend_from_slice(&slope.to_le_bytes());
        expect.extend_from_slice(&mid_price.to_le_bytes());
        expect.extend_from_slice(&(is_open_twap as u8).to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_swap() {
        let amount_in: u64 = 1_000_000;
        let minimum_amount_out: u64 = 500_000;
        let swap_direction: SwapDirection = SwapDirection::SellBase;
        let check = SwapInstruction::Swap(SwapData {
            amount_in,
            minimum_amount_out,
            swap_direction,
        });
        let packed = check.pack();
        let mut expect = vec![1];
        expect.extend_from_slice(&amount_in.to_le_bytes());
        expect.extend_from_slice(&minimum_amount_out.to_le_bytes());
        expect.extend_from_slice(&(swap_direction as u8).to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_deposit() {
        let token_a_amount: u64 = 1_000_000;
        let token_b_amount: u64 = 500_000;
        let min_mint_amount: u64 = 500_000;
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
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_withdraw() {
        let minimum_token_a_amount: u64 = 1_000_000;
        let minimum_token_b_amount: u64 = 500_000;
        let pool_token_amount: u64 = 500_000;
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
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }
}
