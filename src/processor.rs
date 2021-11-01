//! Program state processor

#![allow(clippy::too_many_arguments)]

use std::convert::TryInto;

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use spl_token::{
    instruction::AuthorityType,
    state::{Account, Mint},
};

use crate::{
    admin::process_admin_instruction,
    curve::{Multiplier, PoolState},
    error::SwapError,
    instruction::{
        DepositData, InitializeData, InstructionType, SwapData, SwapDirection, SwapInstruction,
        WithdrawData,
    },
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
    pyth,
    state::{ConfigInfo, LiquidityProvider, SwapInfo},
};

/// Processes an [Instruction](enum.Instruction.html).
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    match InstructionType::check(input) {
        Some(InstructionType::Admin) => process_admin_instruction(program_id, accounts, input),
        Some(InstructionType::Swap) => process_swap_instruction(program_id, accounts, input),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_swap_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = SwapInstruction::unpack(input)?;
    match instruction {
        SwapInstruction::Initialize(InitializeData {
            nonce,
            slope,
            mid_price,
            is_open_twap,
        }) => {
            msg!("Instruction: Initialize");
            process_initialize(program_id, nonce, slope, mid_price, is_open_twap, accounts)
        }
        SwapInstruction::Swap(SwapData {
            amount_in,
            minimum_amount_out,
            swap_direction,
        }) => {
            msg!("Instruction: Swap");
            process_swap(
                program_id,
                amount_in,
                minimum_amount_out,
                swap_direction,
                accounts,
            )
        }
        SwapInstruction::Deposit(DepositData {
            token_a_amount,
            token_b_amount,
            min_mint_amount,
        }) => {
            msg!("Instruction: Deposit");
            process_deposit(
                program_id,
                token_a_amount,
                token_b_amount,
                min_mint_amount,
                accounts,
            )
        }
        SwapInstruction::Withdraw(WithdrawData {
            pool_token_amount,
            minimum_token_a_amount,
            minimum_token_b_amount,
        }) => {
            msg!("Instruction: Withdraw");
            process_withdraw(
                program_id,
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
                accounts,
            )
        }
        SwapInstruction::InitializeLiquidityProvider => {
            msg!("Instruction: Initialize Liquidity user");
            process_init_liquidity_provider(program_id, accounts)
        }
        SwapInstruction::RefreshLiquidityObligation => {
            msg!("Instruction: Refresh liquidity obligation");
            process_refresh_liquidity_obligation(program_id, accounts)
        }
        SwapInstruction::ClaimLiquidityRewards => {
            msg!("Instruction: Claim Liquidity Rewards");
            process_claim_liquidity_rewards(program_id, accounts)
        }
    }
}

fn process_initialize(
    program_id: &Pubkey,
    nonce: u8,
    slope: u64,
    mid_price: u128,
    is_open_twap: bool,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_fee_a_info = next_account_info(account_info_iter)?;
    let admin_fee_b_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    assert_uninitialized::<SwapInfo>(swap_info)?;
    if *authority_info.key != authority_id(program_id, swap_info.key, nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    let token_program_id = *token_program_info.key;
    let destination = unpack_token_account(destination_info, &token_program_id)?;
    let token_a = unpack_token_account(token_a_info, &token_program_id)?;
    let token_b = unpack_token_account(token_b_info, &token_program_id)?;
    let pool_mint = unpack_mint(pool_mint_info, &token_program_id)?;
    let admin_fee_key_a = unpack_token_account(admin_fee_a_info, &token_program_id)?;
    let admin_fee_key_b = unpack_token_account(admin_fee_b_info, &token_program_id)?;
    if *authority_info.key != token_a.owner {
        return Err(SwapError::InvalidOwner.into());
    }
    if *authority_info.key != token_b.owner {
        return Err(SwapError::InvalidOwner.into());
    }
    if *authority_info.key == destination.owner {
        return Err(SwapError::InvalidOutputOwner.into());
    }
    if *authority_info.key == admin_fee_key_a.owner {
        return Err(SwapError::InvalidOutputOwner.into());
    }
    if *authority_info.key == admin_fee_key_b.owner {
        return Err(SwapError::InvalidOutputOwner.into());
    }
    if token_a.mint == token_b.mint {
        return Err(SwapError::RepeatedMint.into());
    }
    if token_a.mint != admin_fee_key_a.mint {
        return Err(SwapError::InvalidAdmin.into());
    }
    if token_b.mint != admin_fee_key_b.mint {
        return Err(SwapError::InvalidAdmin.into());
    }
    if token_b.amount == 0 {
        return Err(SwapError::EmptySupply.into());
    }
    if token_a.amount == 0 {
        return Err(SwapError::EmptySupply.into());
    }
    if token_a.delegate.is_some() {
        return Err(SwapError::InvalidDelegate.into());
    }
    if token_b.delegate.is_some() {
        return Err(SwapError::InvalidDelegate.into());
    }
    if token_a.close_authority.is_some() {
        return Err(SwapError::InvalidCloseAuthority.into());
    }
    if token_b.close_authority.is_some() {
        return Err(SwapError::InvalidCloseAuthority.into());
    }
    if pool_mint.mint_authority.is_some()
        && *authority_info.key != pool_mint.mint_authority.unwrap()
    {
        return Err(SwapError::InvalidOwner.into());
    }
    if pool_mint.freeze_authority.is_some() {
        return Err(SwapError::InvalidFreezeAuthority.into());
    }
    if pool_mint.supply != 0 {
        return Err(SwapError::InvalidSupply.into());
    }
    if Decimal::from_scaled_val(slope as u128).lt(&Decimal::zero())
        || Decimal::from_scaled_val(slope as u128).gt(&Decimal::one())
    {
        return Err(SwapError::InvalidSlope.into());
    }

    // getting price from pyth or initial mid_price
    let market_price = get_market_price_from_pyth(pyth_a_price_info, pyth_b_price_info, clock)
        .unwrap_or_else(|_| Decimal::from_scaled_val(mid_price));

    let mut pool_state = PoolState::new(PoolState {
        market_price,
        slope: Decimal::from_scaled_val(slope.into()),
        base_target: Decimal::zero(),
        quote_target: Decimal::zero(),
        base_reserve: Decimal::zero(),
        quote_reserve: Decimal::zero(),
        multiplier: Multiplier::One,
    })?;

    let mint_amount = pool_state.buy_shares(token_a.amount, token_b.amount, pool_mint.supply)?;

    let block_timestamp_last: u64 = clock.unix_timestamp.try_into().unwrap();
    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    SwapInfo::pack(
        SwapInfo {
            is_initialized: true,
            is_paused: false,
            nonce,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            pool_mint: *pool_mint_info.key,
            token_a_mint: token_a.mint,
            token_b_mint: token_b.mint,
            admin_fee_key_a: *admin_fee_a_info.key,
            admin_fee_key_b: *admin_fee_b_info.key,
            fees: config.fees,
            rewards: config.rewards,
            pool_state,
            is_open_twap,
            block_timestamp_last,
            cumulative_ticks: 0,
            base_price_cumulative_last: Decimal::zero(),
        },
        &mut swap_info.data.borrow_mut(),
    )?;

    token_mint_to(
        swap_info.key,
        token_program_info.clone(),
        pool_mint_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
        nonce,
        mint_amount,
    )?;

    Ok(())
}

fn process_swap(
    program_id: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    swap_direction: SwapDirection,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let swap_destination_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let reward_token_info = next_account_info(account_info_iter)?;
    let reward_mint_info = next_account_info(account_info_iter)?;
    let admin_destination_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }
    let swap_nonce = token_swap.nonce;
    if *swap_authority_info.key != authority_id(program_id, swap_info.key, swap_nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    if !(*swap_source_info.key == token_swap.token_a || *swap_source_info.key == token_swap.token_b)
    {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if !(*swap_destination_info.key == token_swap.token_a
        || *swap_destination_info.key == token_swap.token_b)
    {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *swap_source_info.key == *swap_destination_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if swap_source_info.key == source_info.key || swap_destination_info.key == destination_info.key
    {
        return Err(SwapError::InvalidInput.into());
    }

    let token_program_id = *token_program_info.key;
    let token_a = unpack_token_account(swap_source_info, &token_program_id)?;
    let token_b = unpack_token_account(swap_destination_info, &token_program_id)?;
    let reward_token = unpack_token_account(reward_token_info, &token_program_id)?;
    let reward_mint = unpack_mint(reward_mint_info, &token_program_id)?;

    // TODO: ======== Need check more =========
    let market_nonce = config.bump_seed;
    if *market_authority_info.key != authority_id(program_id, config_info.key, market_nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if config.deltafi_mint != *reward_mint_info.key {
        return Err(SwapError::IncorrectMint.into());
    }
    if reward_token.owner == *market_authority_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if reward_mint.mint_authority.is_some()
        && *market_authority_info.key != reward_mint.mint_authority.unwrap()
    {
        return Err(SwapError::InvalidOwner.into());
    }
    if &reward_token.mint != reward_mint_info.key {
        return Err(SwapError::IncorrectMint.into());
    }

    match swap_direction {
        SwapDirection::SellBase => {
            if *swap_destination_info.key == token_swap.token_a
                && *admin_destination_info.key != token_swap.admin_fee_key_a
            {
                return Err(SwapError::InvalidAdmin.into());
            }
            if *swap_destination_info.key == token_swap.token_b
                && *admin_destination_info.key != token_swap.admin_fee_key_b
            {
                return Err(SwapError::InvalidAdmin.into());
            }
            if token_a.amount < amount_in {
                return Err(SwapError::InsufficientFunds.into());
            }
        }
        SwapDirection::SellQuote => {
            if *swap_destination_info.key == token_swap.token_a
                && *admin_destination_info.key != token_swap.admin_fee_key_b
            {
                return Err(SwapError::InvalidAdmin.into());
            }
            if *swap_destination_info.key == token_swap.token_b
                && *admin_destination_info.key != token_swap.admin_fee_key_a
            {
                return Err(SwapError::InvalidAdmin.into());
            }
            if token_b.amount < amount_in {
                return Err(SwapError::InsufficientFunds.into());
            }
        }
    }

    let (new_market_price, base_price_cumulative_last) =
        get_new_market_price(&mut token_swap, pyth_a_price_info, pyth_b_price_info, clock)?;

    let state = PoolState::new(PoolState {
        market_price: new_market_price,
        ..token_swap.pool_state
    })?;

    let (receive_amount, new_multiplier) = match swap_direction {
        SwapDirection::SellBase => state.sell_base_token(amount_in)?,
        SwapDirection::SellQuote => state.sell_quote_token(amount_in)?,
    };
    let fees = &token_swap.fees;
    let trade_fee = fees.trade_fee(receive_amount)?;
    let admin_fee = fees.admin_trade_fee(trade_fee)?;
    let rewards = &token_swap.rewards;
    let amount_to_reward = rewards.trade_reward_u64(amount_in)?;
    let amount_out = receive_amount
        .checked_sub(trade_fee)
        .ok_or(SwapError::CalculationFailure)?;

    if amount_out < minimum_amount_out {
        return Err(SwapError::ExceededSlippage.into());
    }

    let (base_balance, quote_balance) = match swap_direction {
        SwapDirection::SellBase => (
            token_a
                .amount
                .checked_add(amount_in)
                .ok_or(SwapError::CalculationFailure)?,
            token_b
                .amount
                .checked_sub(amount_out)
                .ok_or(SwapError::CalculationFailure)?,
        ),
        SwapDirection::SellQuote => (
            token_a
                .amount
                .checked_sub(amount_out)
                .ok_or(SwapError::CalculationFailure)?,
            token_b
                .amount
                .checked_add(amount_in)
                .ok_or(SwapError::CalculationFailure)?,
        ),
    };

    token_swap.pool_state = PoolState::new(PoolState {
        base_reserve: Decimal::from(base_balance),
        quote_reserve: Decimal::from(quote_balance),
        multiplier: new_multiplier,
        ..state
    })?;

    token_swap.cumulative_ticks = token_swap
        .cumulative_ticks
        .checked_add(clock.unix_timestamp.try_into().unwrap())
        .ok_or(SwapError::CalculationFailure)?
        .checked_sub(token_swap.block_timestamp_last)
        .ok_or(SwapError::CalculationFailure)?;
    token_swap.block_timestamp_last = clock.unix_timestamp.try_into().unwrap();
    token_swap.base_price_cumulative_last = base_price_cumulative_last;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    match swap_direction {
        SwapDirection::SellBase => {
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                source_info.clone(),
                swap_source_info.clone(),
                user_transfer_authority_info.clone(),
                swap_nonce,
                amount_in,
            )?;
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                swap_destination_info.clone(),
                destination_info.clone(),
                swap_authority_info.clone(),
                swap_nonce,
                amount_out,
            )?;
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                swap_destination_info.clone(),
                admin_destination_info.clone(),
                swap_authority_info.clone(),
                swap_nonce,
                admin_fee,
            )?;
            token_mint_to(
                config_info.key,
                token_program_info.clone(),
                reward_mint_info.clone(),
                reward_token_info.clone(),
                market_authority_info.clone(),
                market_nonce,
                amount_to_reward,
            )?;
        }
        SwapDirection::SellQuote => {
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                destination_info.clone(),
                swap_destination_info.clone(),
                user_transfer_authority_info.clone(),
                swap_nonce,
                amount_in,
            )?;
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                swap_source_info.clone(),
                source_info.clone(),
                swap_authority_info.clone(),
                swap_nonce,
                amount_out,
            )?;
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                swap_source_info.clone(),
                admin_destination_info.clone(),
                swap_authority_info.clone(),
                swap_nonce,
                admin_fee,
            )?;
            token_mint_to(
                config_info.key,
                token_program_info.clone(),
                reward_mint_info.clone(),
                reward_token_info.clone(),
                market_authority_info.clone(),
                market_nonce,
                amount_to_reward,
            )?;
        }
    };

    Ok(())
}

fn process_deposit(
    program_id: &Pubkey,
    token_a_amount: u64,
    token_b_amount: u64,
    min_mint_amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_a_info = next_account_info(account_info_iter)?;
    let source_b_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let liquidity_provider_info = next_account_info(account_info_iter)?;
    let liquidity_owner_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }

    let nonce = token_swap.nonce;
    if *authority_info.key != authority_id(program_id, swap_info.key, nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if *token_a_info.key != token_swap.token_a {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *token_b_info.key != token_swap.token_b {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *pool_mint_info.key != token_swap.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if token_a_info.key == source_a_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if token_b_info.key == source_b_info.key {
        return Err(SwapError::InvalidInput.into());
    }

    let mut liquidity_provider =
        LiquidityProvider::unpack(&liquidity_provider_info.data.borrow_mut())?;
    if liquidity_provider_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }
    if &liquidity_provider.owner != liquidity_owner_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if !liquidity_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    let token_program_id = *token_program_info.key;
    let token_a = unpack_token_account(token_a_info, &token_program_id)?;
    let token_b = unpack_token_account(token_b_info, &token_program_id)?;
    let pool_mint = unpack_mint(pool_mint_info, &token_program_id)?;

    // updating price from pyth price
    let (new_market_price, base_price_cumulative_last) =
        get_new_market_price(&mut token_swap, pyth_a_price_info, pyth_b_price_info, clock)?;

    let mut state = PoolState::new(PoolState {
        market_price: new_market_price,
        ..token_swap.pool_state
    })?;

    let base_balance = token_a_amount
        .checked_add(token_a.amount)
        .ok_or(SwapError::CalculationFailure)?;
    let quote_balance = token_b_amount
        .checked_add(token_b.amount)
        .ok_or(SwapError::CalculationFailure)?;

    let pool_mint_amount = state.buy_shares(base_balance, quote_balance, pool_mint.supply)?;

    if pool_mint_amount < min_mint_amount {
        return Err(SwapError::ExceededSlippage.into());
    }

    liquidity_provider
        .find_or_add_position(*swap_info.key, clock.unix_timestamp)?
        .deposit(pool_mint_amount)?;
    LiquidityProvider::pack(
        liquidity_provider,
        &mut liquidity_provider_info.data.borrow_mut(),
    )?;

    token_swap.pool_state = state;
    token_swap.cumulative_ticks = token_swap
        .cumulative_ticks
        .checked_add(clock.unix_timestamp.try_into().unwrap())
        .ok_or(SwapError::CalculationFailure)?
        .checked_sub(token_swap.block_timestamp_last)
        .ok_or(SwapError::CalculationFailure)?;

    token_swap.block_timestamp_last = clock.unix_timestamp.try_into().unwrap();
    token_swap.base_price_cumulative_last = base_price_cumulative_last;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        source_a_info.clone(),
        token_a_info.clone(),
        user_transfer_authority_info.clone(),
        nonce,
        token_a_amount,
    )?;
    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        source_b_info.clone(),
        token_b_info.clone(),
        user_transfer_authority_info.clone(),
        nonce,
        token_b_amount,
    )?;
    token_mint_to(
        swap_info.key,
        token_program_info.clone(),
        pool_mint_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
        nonce,
        pool_mint_amount,
    )?;

    Ok(())
}

fn process_withdraw(
    program_id: &Pubkey,
    pool_token_amount: u64,
    minimum_token_a_amount: u64,
    minimum_token_b_amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let dest_token_a_info = next_account_info(account_info_iter)?;
    let dest_token_b_info = next_account_info(account_info_iter)?;
    let admin_fee_dest_a_info = next_account_info(account_info_iter)?;
    let admin_fee_dest_b_info = next_account_info(account_info_iter)?;
    let liquidity_provider_info = next_account_info(account_info_iter)?;
    let liquidity_owner_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    let nonce = token_swap.nonce;
    if *authority_info.key != authority_id(program_id, swap_info.key, nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if *token_a_info.key != token_swap.token_a {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *token_b_info.key != token_swap.token_b {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if token_a_info.key == dest_token_a_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if token_b_info.key == dest_token_b_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if *pool_mint_info.key != token_swap.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if *admin_fee_dest_a_info.key != token_swap.admin_fee_key_a {
        return Err(SwapError::InvalidAdmin.into());
    }
    if *admin_fee_dest_b_info.key != token_swap.admin_fee_key_b {
        return Err(SwapError::InvalidAdmin.into());
    }

    let token_program_id = *token_program_info.key;
    let pool_mint = unpack_mint(pool_mint_info, &token_program_id)?;
    if pool_mint.supply == 0 {
        return Err(SwapError::EmptySupply.into());
    }

    let mut liquidity_provider =
        LiquidityProvider::unpack(&liquidity_provider_info.data.borrow_mut())?;
    if liquidity_provider_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }
    if &liquidity_provider.owner != liquidity_owner_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if !liquidity_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    let (new_market_price, base_price_cumulative_last) =
        get_new_market_price(&mut token_swap, pyth_a_price_info, pyth_b_price_info, clock)?;

    let mut state = PoolState::new(PoolState {
        market_price: new_market_price,
        ..token_swap.pool_state
    })?;

    let (base_out_amount, quote_out_amount) = state.sell_shares(
        pool_token_amount,
        minimum_token_a_amount,
        minimum_token_b_amount,
        pool_mint.supply,
    )?;

    let fees = &token_swap.fees;
    let withdraw_fee_base = fees.withdraw_fee(base_out_amount)?;
    let admin_fee_base = fees.admin_withdraw_fee(withdraw_fee_base)?;
    let base_out_amount = base_out_amount
        .checked_sub(withdraw_fee_base)
        .ok_or(SwapError::CalculationFailure)?;

    let withdraw_fee_quote = fees.withdraw_fee(quote_out_amount)?;
    let admin_fee_quote = fees.admin_withdraw_fee(withdraw_fee_quote)?;
    let quote_out_amount = quote_out_amount
        .checked_sub(withdraw_fee_quote)
        .ok_or(SwapError::CalculationFailure)?;

    let (_, position_index) = liquidity_provider.find_position(*swap_info.key)?;
    liquidity_provider.withdraw(pool_token_amount, position_index)?;
    LiquidityProvider::pack(
        liquidity_provider,
        &mut liquidity_provider_info.data.borrow_mut(),
    )?;

    token_swap.pool_state = state;
    token_swap.cumulative_ticks = token_swap
        .cumulative_ticks
        .checked_add(clock.unix_timestamp.try_into().unwrap())
        .ok_or(SwapError::CalculationFailure)?
        .checked_sub(token_swap.block_timestamp_last)
        .ok_or(SwapError::CalculationFailure)?;
    token_swap.block_timestamp_last = clock.unix_timestamp.try_into().unwrap();
    token_swap.base_price_cumulative_last = base_price_cumulative_last;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        token_a_info.clone(),
        dest_token_a_info.clone(),
        authority_info.clone(),
        nonce,
        base_out_amount,
    )?;
    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        token_a_info.clone(),
        admin_fee_dest_a_info.clone(),
        authority_info.clone(),
        nonce,
        admin_fee_base,
    )?;
    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        token_b_info.clone(),
        dest_token_b_info.clone(),
        authority_info.clone(),
        nonce,
        quote_out_amount,
    )?;
    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        token_b_info.clone(),
        admin_fee_dest_b_info.clone(),
        authority_info.clone(),
        nonce,
        admin_fee_quote,
    )?;
    token_burn(
        swap_info.key,
        token_program_info.clone(),
        source_info.clone(),
        pool_mint_info.clone(),
        user_transfer_authority_info.clone(),
        nonce,
        pool_token_amount,
    )?;

    Ok(())
}

fn process_init_liquidity_provider(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let liquidity_provider_info = next_account_info(account_info_iter)?;
    let liquidity_owner_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    if liquidity_provider_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    assert_rent_exempt(rent, liquidity_provider_info)?;
    let mut liquidity_provider =
        assert_uninitialized::<LiquidityProvider>(liquidity_provider_info)?;

    if !liquidity_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    liquidity_provider.init(*liquidity_owner_info.key, vec![]);
    LiquidityProvider::pack(
        liquidity_provider,
        &mut liquidity_provider_info.data.borrow_mut(),
    )?;

    Ok(())
}

fn process_claim_liquidity_rewards(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let liquidity_provider_info = next_account_info(account_info_iter)?;
    let liquidity_owner_info = next_account_info(account_info_iter)?;
    let claim_destination_info = next_account_info(account_info_iter)?;
    let claim_mint_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let market_nonce = config.bump_seed;
    if *market_authority_info.key != authority_id(program_id, config_info.key, market_nonce)? {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    if config.deltafi_mint != *claim_mint_info.key {
        return Err(SwapError::IncorrectMint.into());
    }
    if claim_destination_info.owner == market_authority_info.key {
        return Err(SwapError::InvalidOwner.into());
    }

    let mut liquidity_provider =
        LiquidityProvider::unpack(&liquidity_provider_info.data.borrow_mut())?;
    if liquidity_provider.owner != *liquidity_owner_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if !liquidity_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    let reward_amount = liquidity_provider.claim(*swap_info.key)?;
    LiquidityProvider::pack(
        liquidity_provider,
        &mut liquidity_provider_info.data.borrow_mut(),
    )?;

    token_mint_to(
        config_info.key,
        token_program_info.clone(),
        claim_mint_info.clone(),
        claim_destination_info.clone(),
        market_authority_info.clone(),
        market_nonce,
        reward_amount,
    )?;

    Ok(())
}

fn process_refresh_liquidity_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let swap_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

    if swap_info.owner != program_id {
        msg!("Swap account is not owned by swap token program");
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;

    let lp_price = token_swap.pool_state.get_mid_price()?;
    let _deltafi_price = Decimal::one().try_div(10)?; // Temp value
    let reward_ratio = lp_price.try_div(_deltafi_price)?;

    for liquidity_provider_info in account_info_iter {
        let mut liquidity_provider =
            LiquidityProvider::unpack(&liquidity_provider_info.data.borrow_mut())?;
        let (position, _) = liquidity_provider.find_position(*swap_info.key)?;
        position.calc_and_update_rewards(reward_ratio, clock.unix_timestamp)?;

        LiquidityProvider::pack(
            liquidity_provider,
            &mut liquidity_provider_info.data.borrow_mut(),
        )?;
    }

    Ok(())
}

fn get_new_market_price(
    token_swap: &mut SwapInfo,
    pyth_a_price_info: &AccountInfo,
    pyth_b_price_info: &AccountInfo,
    clock: &Clock,
) -> Result<(Decimal, Decimal), ProgramError> {
    let pool_state = &mut token_swap.pool_state;
    let pool_mid_price = pool_state.get_mid_price()?;
    let block_timestamp_last: u64 = clock.unix_timestamp.try_into().unwrap();
    let mut base_price_cumulative_last = token_swap.base_price_cumulative_last;
    if token_swap.is_open_twap {
        let time_elapsed = block_timestamp_last - token_swap.block_timestamp_last;
        if time_elapsed > 0
            && !pool_state.base_reserve.is_zero()
            && !pool_state.quote_reserve.is_zero()
        {
            base_price_cumulative_last =
                base_price_cumulative_last.try_add(pool_mid_price.try_mul(time_elapsed as u64)?)?;
        }
    }

    let market_price = if let Ok(market_price) =
        get_market_price_from_pyth(pyth_a_price_info, pyth_b_price_info, clock)
    {
        // pyth price
        market_price
    } else if token_swap.is_open_twap {
        // internal oracle price
        base_price_cumulative_last.try_div(block_timestamp_last - token_swap.cumulative_ticks)?
    } else {
        // current pool middle price
        pool_mid_price
    };

    let deviation = if pool_mid_price > market_price {
        pool_mid_price.try_sub(market_price)?
    } else {
        market_price.try_sub(pool_mid_price)?
    };

    Ok((
        if deviation.try_mul(100u64)? > pool_mid_price {
            market_price
        } else {
            pool_mid_price
        },
        base_price_cumulative_last,
    ))
}

fn get_market_price_from_pyth(
    pyth_a_price_info: &AccountInfo,
    pyth_b_price_info: &AccountInfo,
    clock: &Clock,
) -> Result<Decimal, ProgramError> {
    let price_a = get_pyth_price(pyth_a_price_info, clock)?;
    let price_b = get_pyth_price(pyth_b_price_info, clock)?;

    if price_a > price_b {
        price_a.try_div(price_b)
    } else {
        price_b.try_div(price_a)
    }
}

fn _get_pyth_product_quote_currency(
    pyth_product: &pyth::Product,
) -> Result<[u8; 32], ProgramError> {
    const LEN: usize = 14;
    const KEY: &[u8; LEN] = b"quote_currency";

    let mut start = 0;
    while start < pyth::PROD_ATTR_SIZE {
        let mut length = pyth_product.attr[start] as usize;
        start += 1;

        if length == LEN {
            let mut end = start + length;
            if end > pyth::PROD_ATTR_SIZE {
                msg!("Pyth product attribute key length too long");
                return Err(SwapError::InvalidOracleConfig.into());
            }

            let key = &pyth_product.attr[start..end];
            if key == KEY {
                start += length;
                length = pyth_product.attr[start] as usize;
                start += 1;

                end = start + length;
                if length > 32 || end > pyth::PROD_ATTR_SIZE {
                    msg!("Pyth product quote currency value too long");
                    return Err(SwapError::InvalidOracleConfig.into());
                }

                let mut value = [0u8; 32];
                value[0..length].copy_from_slice(&pyth_product.attr[start..end]);
                return Ok(value);
            }
        }

        start += length;
        start += 1 + pyth_product.attr[start] as usize;
    }

    msg!("Pyth product quote currency not found");
    Err(SwapError::InvalidOracleConfig.into())
}

fn get_pyth_price(pyth_price_info: &AccountInfo, clock: &Clock) -> Result<Decimal, ProgramError> {
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 5;

    let pyth_price_data = pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth::load::<pyth::Price>(&pyth_price_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;

    if pyth_price.ptype != pyth::PriceType::Price {
        msg!("Oracle price type is invalid");
        return Err(SwapError::InvalidOracleConfig.into());
    }

    let slots_elapsed = clock
        .slot
        .checked_sub(pyth_price.valid_slot)
        .ok_or(SwapError::CalculationFailure)?;
    if slots_elapsed >= STALE_AFTER_SLOTS_ELAPSED {
        msg!("Oracle price is stale");
        return Err(SwapError::InvalidOracleConfig.into());
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| {
        msg!("Oracle price cannot be negative");
        SwapError::InvalidOracleConfig
    })?;

    // if conf / price > 1% -> volative, do not use pyth price?
    if pyth_price.agg.conf > 0 && price < pyth_price.agg.conf * 100u64 {
        msg!("Pyth suggests market is volatile");
        return Err(SwapError::InvalidOracleConfig.into());
    }

    let market_price = if pyth_price.expo >= 0 {
        let exponent = pyth_price
            .expo
            .try_into()
            .map_err(|_| SwapError::CalculationFailure)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(SwapError::CalculationFailure)?;
        Decimal::from(price).try_mul(zeros)?
    } else {
        let exponent = pyth_price
            .expo
            .checked_abs()
            .ok_or(SwapError::CalculationFailure)?
            .try_into()
            .map_err(|_| SwapError::CalculationFailure)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(SwapError::CalculationFailure)?;
        Decimal::from(price).try_div(decimals)?
    };

    Ok(market_price)
}

/// Assert and unpack account data
pub fn assert_uninitialized<T: Pack + IsInitialized>(
    account_info: &AccountInfo,
) -> Result<T, ProgramError> {
    let account: T = T::unpack_unchecked(&account_info.data.borrow())?;
    if account.is_initialized() {
        Err(SwapError::AlreadyInUse.into())
    } else {
        Ok(account)
    }
}

/// Check if the account has enough lamports to be rent to store state
pub fn assert_rent_exempt(rent: &Rent, account_info: &AccountInfo) -> ProgramResult {
    if !rent.is_exempt(account_info.lamports(), account_info.data_len()) {
        msg!(&rent.minimum_balance(account_info.data_len()).to_string());
        Err(SwapError::NotRentExempt.into())
    } else {
        Ok(())
    }
}

/// Unpacks a spl_token `Mint`.
pub fn unpack_mint(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<Mint, SwapError> {
    if account_info.owner != token_program_id {
        Err(SwapError::IncorrectTokenProgramId)
    } else {
        Mint::unpack(&account_info.data.borrow()).map_err(|_| SwapError::ExpectedMint)
    }
}

/// Issue a spl_token `Transfer` instruction.
fn token_transfer<'a>(
    swap: &Pubkey,
    token_program: AccountInfo<'a>,
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    nonce: u8,
    amount: u64,
) -> Result<(), ProgramError> {
    let swap_bytes = swap.to_bytes();
    let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
    let signers = &[&authority_signature_seeds[..]];
    let ix = spl_token::instruction::transfer(
        token_program.key,
        source.key,
        destination.key,
        authority.key,
        &[],
        amount,
    )?;

    invoke_signed(
        &ix,
        &[source, destination, authority, token_program],
        signers,
    )
}

/// Issue a spl_token `MintTo` instruction.
fn token_mint_to<'a>(
    swap: &Pubkey,
    token_program: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    nonce: u8,
    amount: u64,
) -> Result<(), ProgramError> {
    let swap_bytes = swap.to_bytes();
    let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
    let signers = &[&authority_signature_seeds[..]];
    let ix = spl_token::instruction::mint_to(
        token_program.key,
        mint.key,
        destination.key,
        authority.key,
        &[],
        amount,
    )?;

    invoke_signed(&ix, &[mint, destination, authority, token_program], signers)
}

/// Issue a spl_token `Burn` instruction.
fn token_burn<'a>(
    swap: &Pubkey,
    token_program: AccountInfo<'a>,
    burn_account: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    nonce: u8,
    amount: u64,
) -> ProgramResult {
    let swap_bytes = swap.to_bytes();
    let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
    let signers = &[&authority_signature_seeds[..]];
    let ix = spl_token::instruction::burn(
        token_program.key,
        burn_account.key,
        mint.key,
        authority.key,
        &[],
        amount,
    )?;

    invoke_signed(
        &ix,
        &[burn_account, mint, authority, token_program],
        signers,
    )
}

/// Set account authority
pub fn set_authority<'a>(
    token_program: &AccountInfo<'a>,
    account_to_transfer_ownership: &AccountInfo<'a>,
    new_authority: Option<Pubkey>,
    authority_type: AuthorityType,
    owner: &AccountInfo<'a>,
) -> ProgramResult {
    let ix = spl_token::instruction::set_authority(
        token_program.key,
        account_to_transfer_ownership.key,
        new_authority.as_ref(),
        authority_type,
        owner.key,
        &[],
    )?;
    invoke(
        &ix,
        &[
            account_to_transfer_ownership.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )?;
    Ok(())
}

/// Calculates the authority id by generating a program address.
pub fn authority_id(program_id: &Pubkey, my_info: &Pubkey, nonce: u8) -> Result<Pubkey, SwapError> {
    Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
        .or(Err(SwapError::InvalidProgramAddress))
}

/// Unpacks a spl_token `Account`.
pub fn unpack_token_account(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<Account, ProgramError> {
    if account_info.owner != token_program_id {
        Err(SwapError::IncorrectTokenProgramId.into())
    } else {
        spl_token::state::Account::unpack(&account_info.data.borrow())
            .map_err(|_| SwapError::ExpectedAccount.into())
    }
}
