//! Program state processor

#![allow(clippy::too_many_arguments)]

use std::convert::TryInto;

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use spl_token::state::{Account, Mint};

use crate::{
    admin::process_admin_instruction,
    curve::{PMMState, RState},
    error::SwapError,
    instruction::{
        DepositData, InitializeData, InstructionType, SwapData, SwapInstruction, WithdrawData,
    },
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
    pyth,
    state::{ConfigInfo, LiquidityProvider, SwapInfo},
    utils,
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
            slop,
            mid_price,
            is_open_twap,
        }) => {
            msg!("Instruction: Initialize");
            process_initialize(program_id, nonce, slop, mid_price, is_open_twap, accounts)
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
    slop: u64,
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
    let destination_info = next_account_info(account_info_iter)?; // Destination account to mint LP tokens to
    let pyth_price_info = next_account_info(account_info_iter)?; // pyth price account added : 2021.10.21
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

    // getting price from pyth or initial mid_price
    let market_price = get_pyth_price(pyth_price_info, clock)
        .unwrap_or_else(|_| Decimal::from_scaled_val(mid_price));

    let mut pmm_state = PMMState::new(PMMState {
        market_price,
        slop: Decimal::from_scaled_val(slop.into()),
        base_target: Decimal::zero(),
        quote_target: Decimal::zero(),
        base_reserve: Decimal::zero(),
        quote_reserve: Decimal::zero(),
        r: RState::One,
    })?;

    let mint_amount = pmm_state.buy_shares(token_a.amount, token_b.amount, pool_mint.supply)?;

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
            pmm_state,
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
    swap_direction: u8,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let swap_destination_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let reward_token_info = next_account_info(account_info_iter)?;
    let reward_mint_info = next_account_info(account_info_iter)?;
    let admin_destination_info = next_account_info(account_info_iter)?;
    let pyth_price_info = next_account_info(account_info_iter)?; // pyth price account added : 2021.10.21
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

    // ======== Need check more =========
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
        utils::SWAP_DIRECTION_SELL_BASE => {
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
                return Err(ProgramError::InsufficientFunds);
            }
        }
        utils::SWAP_DIRECTION_SELL_QUOTE => {
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
                return Err(ProgramError::InsufficientFunds);
            }
        }
        _ => {
            return Err(ProgramError::InvalidArgument);
        }
    }

    let (new_market_price, base_price_cumulative_last) =
        get_new_market_price(&mut token_swap, pyth_price_info, clock)?;

    let state = PMMState::new(PMMState {
        market_price: new_market_price,
        ..token_swap.pmm_state
    })?;

    let (receive_amount, new_r) = match swap_direction {
        utils::SWAP_DIRECTION_SELL_BASE => state.sell_base_token(amount_in)?,
        utils::SWAP_DIRECTION_SELL_QUOTE => state.sell_quote_token(amount_in)?,
        _ => {
            return Err(ProgramError::InvalidArgument);
        }
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
        utils::SWAP_DIRECTION_SELL_BASE => (
            token_a
                .amount
                .checked_add(amount_in)
                .ok_or(SwapError::CalculationFailure)?,
            token_b
                .amount
                .checked_sub(amount_out)
                .ok_or(SwapError::CalculationFailure)?,
        ),
        utils::SWAP_DIRECTION_SELL_QUOTE => (
            token_a
                .amount
                .checked_sub(amount_out)
                .ok_or(SwapError::CalculationFailure)?,
            token_b
                .amount
                .checked_add(amount_in)
                .ok_or(SwapError::CalculationFailure)?,
        ),
        _ => {
            return Err(ProgramError::InvalidArgument);
        }
    };

    token_swap.pmm_state = PMMState::new(PMMState {
        base_reserve: Decimal::from(base_balance),
        quote_reserve: Decimal::from(quote_balance),
        r: new_r,
        ..state
    })?;
    token_swap.block_timestamp_last = clock.unix_timestamp.try_into().unwrap();
    token_swap.base_price_cumulative_last = base_price_cumulative_last;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    match swap_direction {
        utils::SWAP_DIRECTION_SELL_BASE => {
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                source_info.clone(),
                swap_source_info.clone(),
                swap_authority_info.clone(),
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
        utils::SWAP_DIRECTION_SELL_QUOTE => {
            token_transfer(
                swap_info.key,
                token_program_info.clone(),
                destination_info.clone(),
                swap_destination_info.clone(),
                swap_authority_info.clone(),
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
        _ => {
            return Err(ProgramError::InvalidArgument);
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
    let source_a_info = next_account_info(account_info_iter)?;
    let source_b_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let pyth_price_info = next_account_info(account_info_iter)?; // pyth price account added : 2021.10.21
    let liquidity_provider_info = next_account_info(account_info_iter)?;
    let liquidity_owner_info = next_account_info(account_info_iter)?;
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
        get_new_market_price(&mut token_swap, pyth_price_info, clock)?;

    let mut state = PMMState::new(PMMState {
        market_price: new_market_price,
        ..token_swap.pmm_state
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

    token_swap.pmm_state = state;
    token_swap.block_timestamp_last = clock.unix_timestamp.try_into().unwrap();
    token_swap.base_price_cumulative_last = base_price_cumulative_last;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        source_a_info.clone(),
        token_a_info.clone(),
        authority_info.clone(),
        nonce,
        token_a_amount,
    )?;
    token_transfer(
        swap_info.key,
        token_program_info.clone(),
        source_b_info.clone(),
        token_b_info.clone(),
        authority_info.clone(),
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
    let pyth_price_info = next_account_info(account_info_iter)?;
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
        return Err(SwapError::EmptyPool.into());
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
        get_new_market_price(&mut token_swap, pyth_price_info, clock)?;

    let mut state = PMMState::new(PMMState {
        market_price: new_market_price,
        ..token_swap.pmm_state
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

    token_swap.pmm_state = state;
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
        authority_info.clone(),
        nonce,
        pool_token_amount,
    )?;

    Ok(())
}

fn process_init_liquidity_provider(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let liquidity_provider_info = next_account_info(account_info_iter)?;
    let liquidity_owner_info = next_account_info(account_info_iter)?;

    let mut liquidity_provider =
        assert_uninitialized::<LiquidityProvider>(liquidity_provider_info)?;
    if liquidity_provider_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

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

    let (position, _) = liquidity_provider.find_position(*swap_info.key)?;
    let rewards_owed = position.rewards_owed;
    position.claim_rewards()?;

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
        rewards_owed,
    )?;

    Ok(())
}

fn process_refresh_liquidity_obligation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let swap_info = next_account_info(account_info_iter)?;
    let clock_sysvar_info = next_account_info(account_info_iter)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    if swap_info.owner != program_id {
        msg!("Swap account is not owned by swap token program");
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let clock = Clock::from_account_info(clock_sysvar_info)?;

    let lp_price = token_swap.pmm_state.get_mid_price()?;
    let _deltafi_price = Decimal::one().try_div(10)?; // Temp value
    let reward_ratio = lp_price.try_div(_deltafi_price)?;

    for liquidity_provider_info in account_info_iter {
        let mut liquidity_provider =
            LiquidityProvider::unpack(&liquidity_provider_info.data.borrow_mut())?;
        let (position, _) = liquidity_provider.find_position(*swap_info.key)?;

        let rewards_unit = token_swap.rewards.liquidity_reward_u64(
            reward_ratio
                .try_mul(position.liquidity_amount)?
                .try_floor_u64()?,
        )?;
        position.calc_and_update_rewards(rewards_unit, clock.unix_timestamp)?;

        LiquidityProvider::pack(
            liquidity_provider,
            &mut liquidity_provider_info.data.borrow_mut(),
        )?;
    }

    Ok(())
}

fn get_new_market_price(
    token_swap: &mut SwapInfo,
    pyth_price_info: &AccountInfo,
    clock: &Clock,
) -> Result<(Decimal, Decimal), ProgramError> {
    let pmm_state = &mut token_swap.pmm_state;
    let mid_price = pmm_state.get_mid_price()?;
    let block_timestamp_last: u64 = clock.unix_timestamp.try_into().unwrap();
    let mut base_price_cumulative_last = token_swap.base_price_cumulative_last;
    if token_swap.is_open_twap {
        let time_elapsed = block_timestamp_last - token_swap.block_timestamp_last;
        if time_elapsed > 0
            && !pmm_state.base_reserve.is_zero()
            && !pmm_state.quote_reserve.is_zero()
        {
            base_price_cumulative_last =
                base_price_cumulative_last.try_add(mid_price.try_mul(time_elapsed as u64)?)?;
        }
    }

    let market_price = if let Ok(market_price) = get_pyth_price(pyth_price_info, clock) {
        market_price
    } else if token_swap.is_open_twap {
        base_price_cumulative_last.try_div(block_timestamp_last - token_swap.cumulative_ticks)?
    } else {
        mid_price
    };

    let deviation = if mid_price > market_price {
        mid_price.try_sub(market_price)?
    } else {
        market_price.try_sub(mid_price)?
    };

    Ok((
        if deviation.try_mul(100u64)? > mid_price {
            market_price
        } else {
            mid_price
        },
        base_price_cumulative_last,
    ))
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
) -> Result<(), ProgramError> {
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

/// Calculates the authority id by generating a program address.
pub fn authority_id(program_id: &Pubkey, my_info: &Pubkey, nonce: u8) -> Result<Pubkey, SwapError> {
    Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
        .or(Err(SwapError::InvalidProgramAddress))
}

/// Unpacks a spl_token `Account`.
pub fn unpack_token_account(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<Account, SwapError> {
    if account_info.owner != token_program_id {
        Err(SwapError::IncorrectTokenProgramId)
    } else {
        spl_token::state::Account::unpack(&account_info.data.borrow())
            .map_err(|_| SwapError::ExpectedAccount)
    }
}

#[cfg(feature = "test-bpf")]
mod tests {
    use solana_sdk::account::Account;
    use spl_token::{
        error::TokenError,
        instruction::{approve, mint_to, revoke, set_authority, AuthorityType},
    };

    use super::*;
    use crate::{
        curve_1::MIN_AMP,
        fees::Fees,
        instruction::{
            deposit, farm_deposit, farm_emergency_withdraw, farm_withdraw, swap, withdraw,
            withdraw_one,
        },
        rewards::Rewards,
        utils::{
            test_utils::*, CURVE_PMM, DEFAULT_BASE_POINT, DEFAULT_TOKEN_DECIMALS,
            SWAP_DIRECTION_SELL_BASE, SWAP_DIRECTION_SELL_QUOTE,
        },
    };

    /// Initial amount of pool tokens for swap contract, hard-coded to something
    /// "sensible" given a maximum of u64.
    /// Note that on Ethereum, Uniswap uses the geometric mean of all provided
    /// input amounts, and Balancer uses 100 * 10 ^ 18.
    const INITIAL_SWAP_POOL_AMOUNT: u64 = 1_000_000_000;

    #[test]
    fn test_token_program_id_error() {
        let swap_key = pubkey_rand();
        let mut mint = (pubkey_rand(), Account::default());
        let mut destination = (pubkey_rand(), Account::default());
        let token_program = (spl_token::id(), Account::default());
        let (authority_key, nonce) =
            Pubkey::find_program_address(&[&swap_key.to_bytes()[..]], &SWAP_PROGRAM_ID);
        let mut authority = (authority_key, Account::default());
        let swap_bytes = swap_key.to_bytes();
        let authority_signature_seeds = [&swap_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = mint_to(
            &token_program.0,
            &mint.0,
            &destination.0,
            &authority.0,
            &[],
            10,
        )
        .unwrap();
        let mint = (&mut mint).into();
        let destination = (&mut destination).into();
        let authority = (&mut authority).into();

        let err = invoke_signed(&ix, &[mint, destination, authority], signers).unwrap_err();
        assert_eq!(err, ProgramError::InvalidAccountData);
    }

    #[test]
    fn test_initialize() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP;
        let token_a_amount = 1000;
        let token_b_amount = 2000;
        let pool_token_amount = 10;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();

        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount,
            token_b_amount,
            default_k(),
            default_i(),
            utils::TWAP_OPENED,
            utils::CURVE_PMM,
        );

        // wrong nonce for authority_key
        {
            let old_nonce = accounts.nonce;
            accounts.nonce = old_nonce - 1;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.initialize_swap()
            );
            accounts.nonce = old_nonce;
        }

        // uninitialized token a account
        {
            let old_account = accounts.token_a_account;
            accounts.token_a_account = Account::default();
            assert_eq!(
                Err(SwapError::ExpectedAccount.into()),
                accounts.initialize_swap()
            );
            accounts.token_a_account = old_account;
        }

        // uninitialized token b account
        {
            let old_account = accounts.token_b_account;
            accounts.token_b_account = Account::default();
            assert_eq!(
                Err(SwapError::ExpectedAccount.into()),
                accounts.initialize_swap()
            );
            accounts.token_b_account = old_account;
        }

        // uninitialized pool mint
        {
            let old_account = accounts.pool_mint_account;
            accounts.pool_mint_account = Account::default();
            assert_eq!(
                Err(SwapError::ExpectedMint.into()),
                accounts.initialize_swap()
            );
            accounts.pool_mint_account = old_account;
        }

        // token A account owner is not swap authority
        {
            let (_token_a_key, token_a_account) = mint_token(
                &spl_token::id(),
                &accounts.token_a_mint_key,
                &mut accounts.token_a_mint_account,
                &user_key,
                &user_key,
                0,
            );
            let old_account = accounts.token_a_account;
            accounts.token_a_account = token_a_account;
            assert_eq!(
                Err(SwapError::InvalidOwner.into()),
                accounts.initialize_swap()
            );
            accounts.token_a_account = old_account;
        }

        // token B account owner is not swap authority
        {
            let (_token_b_key, token_b_account) = mint_token(
                &spl_token::id(),
                &accounts.token_b_mint_key,
                &mut accounts.token_b_mint_account,
                &user_key,
                &user_key,
                0,
            );
            let old_account = accounts.token_b_account;
            accounts.token_b_account = token_b_account;
            assert_eq!(
                Err(SwapError::InvalidOwner.into()),
                accounts.initialize_swap()
            );
            accounts.token_b_account = old_account;
        }

        // pool token account owner is swap authority
        {
            let (_pool_token_key, pool_token_account) = mint_token(
                &spl_token::id(),
                &accounts.pool_mint_key,
                &mut accounts.pool_mint_account,
                &accounts.authority_key,
                &accounts.authority_key,
                0,
            );
            let old_account = accounts.pool_token_account;
            accounts.pool_token_account = pool_token_account;
            assert_eq!(
                Err(SwapError::InvalidOutputOwner.into()),
                accounts.initialize_swap()
            );
            accounts.pool_token_account = old_account;
        }

        // deltafi token account owner is swap authority
        {
            let (_deltafi_token_key, deltafi_token_account) = mint_token(
                &spl_token::id(),
                &accounts.deltafi_mint_key,
                &mut accounts.deltafi_mint_account,
                &accounts.authority_key,
                &accounts.authority_key,
                0,
            );
            let old_account = accounts.deltafi_token_account;
            accounts.deltafi_token_account = deltafi_token_account;
            assert_eq!(
                Err(SwapError::InvalidOutputOwner.into()),
                accounts.initialize_swap(),
            );
            accounts.deltafi_token_account = old_account;
        }

        // pool mint authority is not swap authority
        {
            let (_pool_mint_key, pool_mint_account) =
                create_mint(&spl_token::id(), &user_key, DEFAULT_TOKEN_DECIMALS, None);
            let old_mint = accounts.pool_mint_account;
            accounts.pool_mint_account = pool_mint_account;
            assert_eq!(
                Err(SwapError::InvalidOwner.into()),
                accounts.initialize_swap()
            );
            accounts.pool_mint_account = old_mint;
        }

        // pool mint token has freeze authority
        {
            let (_pool_mint_key, pool_mint_account) = create_mint(
                &spl_token::id(),
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                Some(&user_key),
            );
            let old_mint = accounts.pool_mint_account;
            accounts.pool_mint_account = pool_mint_account;
            assert_eq!(
                Err(SwapError::InvalidFreezeAuthority.into()),
                accounts.initialize_swap()
            );
            accounts.pool_mint_account = old_mint;
        }

        // empty token A account
        {
            let (_token_a_key, token_a_account) = mint_token(
                &spl_token::id(),
                &accounts.token_a_mint_key,
                &mut accounts.token_a_mint_account,
                &user_key,
                &accounts.authority_key,
                0,
            );
            let old_account = accounts.token_a_account;
            accounts.token_a_account = token_a_account;
            assert_eq!(
                Err(SwapError::EmptySupply.into()),
                accounts.initialize_swap()
            );
            accounts.token_a_account = old_account;
        }

        // empty token B account
        {
            let (_token_b_key, token_b_account) = mint_token(
                &spl_token::id(),
                &accounts.token_b_mint_key,
                &mut accounts.token_b_mint_account,
                &user_key,
                &accounts.authority_key,
                0,
            );
            let old_account = accounts.token_b_account;
            accounts.token_b_account = token_b_account;
            assert_eq!(
                Err(SwapError::EmptySupply.into()),
                accounts.initialize_swap()
            );
            accounts.token_b_account = old_account;
        }

        // invalid pool tokens
        {
            let old_mint = accounts.pool_mint_account;
            let old_pool_account = accounts.pool_token_account;

            let (_pool_mint_key, pool_mint_account) = create_mint(
                &spl_token::id(),
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            accounts.pool_mint_account = pool_mint_account;

            let (_empty_pool_token_key, empty_pool_token_account) = mint_token(
                &spl_token::id(),
                &accounts.pool_mint_key,
                &mut accounts.pool_mint_account,
                &accounts.authority_key,
                &user_key,
                0,
            );

            let (_pool_token_key, pool_token_account) = mint_token(
                &spl_token::id(),
                &accounts.pool_mint_key,
                &mut accounts.pool_mint_account,
                &accounts.authority_key,
                &user_key,
                pool_token_amount,
            );

            // non-empty pool token account
            accounts.pool_token_account = pool_token_account;
            assert_eq!(
                Err(SwapError::InvalidSupply.into()),
                accounts.initialize_swap()
            );

            // pool tokens already in circulation
            accounts.pool_token_account = empty_pool_token_account;
            assert_eq!(
                Err(SwapError::InvalidSupply.into()),
                accounts.initialize_swap()
            );

            accounts.pool_mint_account = old_mint;
            accounts.pool_token_account = old_pool_account;
        }

        // token A account is delegated
        {
            do_process_instruction(
                approve(
                    &spl_token::id(),
                    &accounts.token_a_key,
                    &user_key,
                    &accounts.authority_key,
                    &[],
                    1,
                )
                .unwrap(),
                vec![
                    &mut accounts.token_a_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();
            assert_eq!(
                Err(SwapError::InvalidDelegate.into()),
                accounts.initialize_swap()
            );

            do_process_instruction(
                revoke(
                    &spl_token::id(),
                    &accounts.token_a_key,
                    &accounts.authority_key,
                    &[],
                )
                .unwrap(),
                vec![&mut accounts.token_a_account, &mut Account::default()],
            )
            .unwrap();
        }

        // token B account is delegated
        {
            do_process_instruction(
                approve(
                    &spl_token::id(),
                    &accounts.token_b_key,
                    &user_key,
                    &accounts.authority_key,
                    &[],
                    1,
                )
                .unwrap(),
                vec![
                    &mut accounts.token_b_account,
                    &mut Account::default(),
                    &mut Account::default(),
                ],
            )
            .unwrap();
            assert_eq!(
                Err(SwapError::InvalidDelegate.into()),
                accounts.initialize_swap()
            );

            do_process_instruction(
                revoke(
                    &spl_token::id(),
                    &accounts.token_b_key,
                    &accounts.authority_key,
                    &[],
                )
                .unwrap(),
                vec![&mut accounts.token_b_account, &mut Account::default()],
            )
            .unwrap();
        }

        // token A account has close authority
        {
            do_process_instruction(
                set_authority(
                    &spl_token::id(),
                    &accounts.token_a_key,
                    Some(&user_key),
                    AuthorityType::CloseAccount,
                    &accounts.authority_key,
                    &[],
                )
                .unwrap(),
                vec![&mut accounts.token_a_account, &mut Account::default()],
            )
            .unwrap();
            assert_eq!(
                Err(SwapError::InvalidCloseAuthority.into()),
                accounts.initialize_swap()
            );

            do_process_instruction(
                set_authority(
                    &spl_token::id(),
                    &accounts.token_a_key,
                    None,
                    AuthorityType::CloseAccount,
                    &user_key,
                    &[],
                )
                .unwrap(),
                vec![&mut accounts.token_a_account, &mut Account::default()],
            )
            .unwrap();
        }

        // token B account has close authority
        {
            do_process_instruction(
                set_authority(
                    &spl_token::id(),
                    &accounts.token_b_key,
                    Some(&user_key),
                    AuthorityType::CloseAccount,
                    &accounts.authority_key,
                    &[],
                )
                .unwrap(),
                vec![&mut accounts.token_b_account, &mut Account::default()],
            )
            .unwrap();
            assert_eq!(
                Err(SwapError::InvalidCloseAuthority.into()),
                accounts.initialize_swap()
            );

            do_process_instruction(
                set_authority(
                    &spl_token::id(),
                    &accounts.token_b_key,
                    None,
                    AuthorityType::CloseAccount,
                    &user_key,
                    &[],
                )
                .unwrap(),
                vec![&mut accounts.token_b_account, &mut Account::default()],
            )
            .unwrap();
        }

        // mismatched admin mints
        {
            let (wrong_admin_fee_key, wrong_admin_fee_account) = mint_token(
                &spl_token::id(),
                &accounts.pool_mint_key,
                &mut accounts.pool_mint_account,
                &accounts.authority_key,
                &user_key,
                0,
            );

            // wrong admin_fee_key_a
            let old_admin_fee_account_a = accounts.admin_fee_a_account;
            let old_admin_fee_key_a = accounts.admin_fee_a_key;
            accounts.admin_fee_a_account = wrong_admin_fee_account.clone();
            accounts.admin_fee_a_key = wrong_admin_fee_key;

            assert_eq!(
                Err(SwapError::InvalidAdmin.into()),
                accounts.initialize_swap()
            );

            accounts.admin_fee_a_account = old_admin_fee_account_a;
            accounts.admin_fee_a_key = old_admin_fee_key_a;

            // wrong admin_fee_key_b
            let old_admin_fee_account_b = accounts.admin_fee_b_account;
            let old_admin_fee_key_b = accounts.admin_fee_b_key;
            accounts.admin_fee_b_account = wrong_admin_fee_account;
            accounts.admin_fee_b_key = wrong_admin_fee_key;

            assert_eq!(
                Err(SwapError::InvalidAdmin.into()),
                accounts.initialize_swap()
            );

            accounts.admin_fee_b_account = old_admin_fee_account_b;
            accounts.admin_fee_b_key = old_admin_fee_key_b;
        }

        // create swap with same token A and B
        {
            let (_token_a_repeat_key, token_a_repeat_account) = mint_token(
                &spl_token::id(),
                &accounts.token_a_mint_key,
                &mut accounts.token_a_mint_account,
                &user_key,
                &accounts.authority_key,
                10,
            );
            let old_account = accounts.token_b_account;
            accounts.token_b_account = token_a_repeat_account;
            assert_eq!(
                Err(SwapError::RepeatedMint.into()),
                accounts.initialize_swap()
            );
            accounts.token_b_account = old_account;
        }

        // create valid swap
        accounts.initialize_swap().unwrap();

        // create again
        {
            assert_eq!(
                Err(SwapError::AlreadyInUse.into()),
                accounts.initialize_swap()
            );
        }
        let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
        assert!(swap_info.is_initialized);
        assert!(!swap_info.is_paused);
        assert_eq!(swap_info.nonce, accounts.nonce);
        assert_eq!(swap_info.initial_amp_factor, amp_factor);
        assert_eq!(swap_info.target_amp_factor, amp_factor);
        assert_eq!(swap_info.start_ramp_ts, ZERO_TS);
        assert_eq!(swap_info.stop_ramp_ts, ZERO_TS);
        assert_eq!(swap_info.token_a, accounts.token_a_key);
        assert_eq!(swap_info.token_b, accounts.token_b_key);
        assert_eq!(swap_info.pool_mint, accounts.pool_mint_key);
        assert_eq!(swap_info.token_a_mint, accounts.token_a_mint_key);
        assert_eq!(swap_info.token_b_mint, accounts.token_b_mint_key);
        assert_eq!(swap_info.deltafi_token, accounts.deltafi_token_key);
        assert_eq!(swap_info.deltafi_mint, accounts.deltafi_mint_key);
        assert_eq!(swap_info.admin_fee_key_a, accounts.admin_fee_a_key);
        assert_eq!(swap_info.admin_fee_key_b, accounts.admin_fee_b_key);
        assert_eq!(swap_info.fees, DEFAULT_TEST_FEES);
        assert_eq!(swap_info.pmm_state.k, default_k());
        assert_eq!(swap_info.pmm_state.i, default_i());
        let token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
        assert_eq!(token_a.amount, token_a_amount);
        let token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
        assert_eq!(token_b.amount, token_b_amount);
        let pool_account = utils::unpack_token_account(&accounts.pool_token_account.data).unwrap();
        let pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();
        assert_eq!(pool_mint.supply, pool_account.amount);
    }
    #[test]
    fn test_deposit() {
        let user_key = pubkey_rand();
        let depositor_key = pubkey_rand();
        let amp_factor = MIN_AMP;
        let token_a_amount = 100;
        let token_b_amount = 10000;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount,
            token_b_amount,
            default_k(),
            default_i(),
            utils::TWAP_OPENED,
            utils::CURVE_PMM,
        );

        let deposit_a = token_a_amount / 10;
        let deposit_b = token_b_amount / 10;
        let min_mint_amount = 0;

        // swap not initialized
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }

        accounts.initialize_swap().unwrap();

        // wrong nonce for authority_key
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            let old_authority = accounts.authority_key;
            let (bad_authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.swap_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = bad_authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
            accounts.authority_key = old_authority;
        }

        // not enough token A
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &depositor_key,
                deposit_a / 2,
                deposit_b,
                0,
            );
            assert_eq!(
                Err(TokenError::InsufficientFunds.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }

        // not enough token B
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &depositor_key,
                deposit_a,
                deposit_b / 2,
                0,
            );
            assert_eq!(
                Err(TokenError::InsufficientFunds.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }

        // wrong swap token accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_b_key,
                    &mut token_b_account,
                    &token_a_key,
                    &mut token_a_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }

        // wrong pool token account
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                mut _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            let (
                wrong_token_key,
                mut wrong_token_account,
                _token_b_key,
                mut _token_b_account,
                _pool_key,
                mut _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &wrong_token_key,
                    &mut wrong_token_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }

        // no approval
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            assert_eq!(
                Err(TokenError::OwnerMismatch.into()),
                do_process_instruction(
                    deposit(
                        &SWAP_PROGRAM_ID,
                        &spl_token::id(),
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &token_b_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        &accounts.pyth_key,
                        deposit_a,
                        deposit_b,
                        min_mint_amount,
                        utils::CURVE_PMM,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut token_a_account,
                        &mut token_b_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut accounts.pool_mint_account,
                        &mut pool_account,
                        &mut accounts.pyth_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                )
            );
        }

        // wrong token program id
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            let wrong_key = pubkey_rand();
            assert_eq!(
                Err(ProgramError::IncorrectProgramId),
                do_process_instruction(
                    deposit(
                        &SWAP_PROGRAM_ID,
                        &wrong_key,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &token_b_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        &accounts.pyth_key,
                        deposit_a,
                        deposit_b,
                        min_mint_amount,
                        utils::CURVE_PMM,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut token_a_account,
                        &mut token_b_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut accounts.pool_mint_account,
                        &mut pool_account,
                        &mut accounts.pyth_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                )
            );
        }

        // wrong swap token accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);

            let old_a_key = accounts.token_a_key;
            let old_a_account = accounts.token_a_account;

            accounts.token_a_key = token_a_key;
            accounts.token_a_account = token_a_account.clone();

            // wrong swap token a account
            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );

            accounts.token_a_key = old_a_key;
            accounts.token_a_account = old_a_account;

            let old_b_key = accounts.token_b_key;
            let old_b_account = accounts.token_b_account;

            accounts.token_b_key = token_b_key;
            accounts.token_b_account = token_b_account.clone();

            // wrong swap token b account
            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );

            accounts.token_b_key = old_b_key;
            accounts.token_b_account = old_b_account;
        }

        // wrong mint
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            let (pool_mint_key, pool_mint_account) = create_mint(
                &spl_token::id(),
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let old_pool_key = accounts.pool_mint_key;
            let old_pool_account = accounts.pool_mint_account;
            accounts.pool_mint_key = pool_mint_key;
            accounts.pool_mint_account = pool_mint_account;

            assert_eq!(
                Err(SwapError::IncorrectMint.into()),
                accounts.deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );

            accounts.pool_mint_key = old_pool_key;
            accounts.pool_mint_account = old_pool_account;
        }

        // slippage exceeeded
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            // min mint_amount in too high
            let high_min_mint_amount = 10000000000000;
            assert_eq!(
                Err(SwapError::ExceededSlippage.into()),
                accounts.deposit(
                    &depositor_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    deposit_a,
                    deposit_b,
                    high_min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }

        // correctly deposit
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            accounts
                .deposit(
                    &depositor_key,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    &pool_key,
                    &mut pool_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
                .unwrap();

            let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
            assert_eq!(swap_token_a.amount, deposit_a + token_a_amount);
            let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
            assert_eq!(swap_token_b.amount, deposit_b + token_b_amount);
            let token_a = utils::unpack_token_account(&token_a_account.data).unwrap();
            assert_eq!(token_a.amount, 0);
            let token_b = utils::unpack_token_account(&token_b_account.data).unwrap();
            assert_eq!(token_b.amount, 0);
            let pool_account = utils::unpack_token_account(&pool_account.data).unwrap();
            let swap_pool_account =
                utils::unpack_token_account(&accounts.pool_token_account.data).unwrap();
            let pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();
            // XXX: Revisit and make sure amount of LP tokens minted is corrected.
            assert_eq!(
                pool_mint.supply,
                pool_account.amount + swap_pool_account.amount
            );
            assert_eq!(swap_token_a.amount, 110);
            assert_eq!(swap_token_b.amount, 11000);
            assert_eq!(pool_mint.supply, 110);
            assert_eq!(swap_pool_account.amount, 100);
        }

        // Pool is paused
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &depositor_key, deposit_a, deposit_b, 0);
            // Pause pool
            accounts.pause().unwrap();

            assert_eq!(
                Err(SwapError::IsPaused.into()),
                accounts.deposit(
                    &depositor_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    deposit_a,
                    deposit_b,
                    min_mint_amount,
                    utils::CURVE_PMM,
                )
            );
        }
    }

    #[test]
    fn test_withdraw() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP;
        let token_a_amount = 1000;
        let token_b_amount = 2000;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount,
            token_b_amount,
            default_k(),
            default_i(),
            utils::TWAP_OPENED,
            utils::CURVE_PMM,
        );
        let withdrawer_key = pubkey_rand();
        let initial_a = token_a_amount / 10;
        let initial_b = token_b_amount / 10;
        let initial_pool = INITIAL_SWAP_POOL_AMOUNT;
        let withdraw_amount = initial_pool / 4;
        let minimum_a_amount = initial_a / 40;
        let minimum_b_amount = initial_b / 40;

        // swap not initialized
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &withdrawer_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );
        }

        accounts.initialize_swap().unwrap();

        // wrong nonce for authority_key
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &withdrawer_key, initial_a, initial_b, 0);
            let old_authority = accounts.authority_key;
            let (bad_authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.swap_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = bad_authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );
            accounts.authority_key = old_authority;
        }

        // not enough pool tokens
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount / 2,
            );
            assert_eq!(
                Err(TokenError::InsufficientFunds.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount / 2,
                    minimum_b_amount / 2,
                )
            );
        }

        // wrong token a / b accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_b_key,
                    &mut token_b_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );
        }

        // wrong admin a / b accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            let (
                wrong_admin_a_key,
                wrong_admin_a_account,
                wrong_admin_b_key,
                wrong_admin_b_account,
                _pool_key,
                mut _pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );

            let old_admin_a_key = accounts.admin_fee_a_key;
            let old_admin_a_account = accounts.admin_fee_a_account;
            accounts.admin_fee_a_key = wrong_admin_a_key;
            accounts.admin_fee_a_account = wrong_admin_a_account;

            assert_eq!(
                Err(SwapError::InvalidAdmin.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_b_key,
                    &mut token_b_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );

            accounts.admin_fee_a_key = old_admin_a_key;
            accounts.admin_fee_a_account = old_admin_a_account;

            let old_admin_b_key = accounts.admin_fee_b_key;
            let old_admin_b_account = accounts.admin_fee_b_account;
            accounts.admin_fee_b_key = wrong_admin_b_key;
            accounts.admin_fee_b_account = wrong_admin_b_account;

            assert_eq!(
                Err(SwapError::InvalidAdmin.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_b_key,
                    &mut token_b_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );

            accounts.admin_fee_b_key = old_admin_b_key;
            accounts.admin_fee_b_account = old_admin_b_account;
        }

        // wrong pool token account
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            let (
                wrong_pool_key,
                mut wrong_pool_account,
                _token_b_key,
                _token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                withdraw_amount,
                initial_b,
                withdraw_amount,
            );
            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &wrong_pool_key,
                    &mut wrong_pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );
        }

        // no approval
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &withdrawer_key, 0, 0, withdraw_amount);
            assert_eq!(
                Err(TokenError::OwnerMismatch.into()),
                do_process_instruction(
                    withdraw(
                        &SWAP_PROGRAM_ID,
                        &spl_token::id(),
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_a_key,
                        &token_b_key,
                        &accounts.admin_fee_a_key,
                        &accounts.admin_fee_b_key,
                        withdraw_amount,
                        minimum_a_amount,
                        minimum_b_amount,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut accounts.pool_mint_account,
                        &mut pool_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut token_a_account,
                        &mut token_b_account,
                        &mut accounts.admin_fee_a_account,
                        &mut accounts.admin_fee_b_account,
                        &mut Account::default(),
                    ],
                )
            );
        }

        // wrong token program id
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            let wrong_key = pubkey_rand();
            assert_eq!(
                Err(ProgramError::IncorrectProgramId),
                do_process_instruction(
                    withdraw(
                        &SWAP_PROGRAM_ID,
                        &wrong_key,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_a_key,
                        &token_b_key,
                        &accounts.admin_fee_a_key,
                        &accounts.admin_fee_b_key,
                        withdraw_amount,
                        minimum_a_amount,
                        minimum_b_amount,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut accounts.pool_mint_account,
                        &mut pool_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut token_a_account,
                        &mut token_b_account,
                        &mut accounts.admin_fee_a_account,
                        &mut accounts.admin_fee_b_account,
                        &mut Account::default(),
                    ],
                )
            );
        }

        // wrong swap token accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );

            let old_a_key = accounts.token_a_key;
            let old_a_account = accounts.token_a_account;

            accounts.token_a_key = token_a_key;
            accounts.token_a_account = token_a_account.clone();

            // wrong swap token a account
            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );

            accounts.token_a_key = old_a_key;
            accounts.token_a_account = old_a_account;

            let old_b_key = accounts.token_b_key;
            let old_b_account = accounts.token_b_account;

            accounts.token_b_key = token_b_key;
            accounts.token_b_account = token_b_account.clone();

            // wrong swap token b account
            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );

            accounts.token_b_key = old_b_key;
            accounts.token_b_account = old_b_account;
        }

        // wrong mint
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );
            let (pool_mint_key, pool_mint_account) = create_mint(
                &spl_token::id(),
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let old_pool_key = accounts.pool_mint_key;
            let old_pool_account = accounts.pool_mint_account;
            accounts.pool_mint_key = pool_mint_key;
            accounts.pool_mint_account = pool_mint_account;

            assert_eq!(
                Err(SwapError::IncorrectMint.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
            );

            accounts.pool_mint_key = old_pool_key;
            accounts.pool_mint_account = old_pool_account;
        }

        // slippage exceeeded
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );
            // minimum A amount out too high
            assert_eq!(
                Err(SwapError::ExceededSlippage.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount * 30, // XXX: 10 -> 30: Revisit this slippage multiplier
                    minimum_b_amount,
                )
            );
            // minimum B amount out too high
            assert_eq!(
                Err(SwapError::ExceededSlippage.into()),
                accounts.withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount * 30, // XXX: 10 -> 30; Revisit this splippage multiplier
                )
            );
        }

        // correct withdrawal
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );

            accounts
                .withdraw(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_a_amount,
                    minimum_b_amount,
                )
                .unwrap();

            let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
            let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
            let pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();
            let pool_converter = PoolTokenConverter {
                supply: U256::from(pool_mint.supply),
                token_a: U256::from(swap_token_a.amount),
                token_b: U256::from(swap_token_b.amount),
                fees: &DEFAULT_TEST_FEES,
            };

            let (withdrawn_a, admin_fee_a) = pool_converter
                .token_a_rate(U256::from(withdraw_amount))
                .unwrap();
            let withrawn_total_a = U256::to_u64(withdrawn_a + admin_fee_a).unwrap();
            assert_eq!(swap_token_a.amount, token_a_amount - withrawn_total_a);
            let (withdrawn_b, admin_fee_b) = pool_converter
                .token_b_rate(U256::from(withdraw_amount))
                .unwrap();
            let withrawn_total_b = U256::to_u64(withdrawn_b + admin_fee_b).unwrap();
            assert_eq!(swap_token_b.amount, token_b_amount - withrawn_total_b);
            let token_a = utils::unpack_token_account(&token_a_account.data).unwrap();
            assert_eq!(
                token_a.amount,
                initial_a + U256::to_u64(withdrawn_a).unwrap()
            );
            let token_b = utils::unpack_token_account(&token_b_account.data).unwrap();
            assert_eq!(
                token_b.amount,
                initial_b + U256::to_u64(withdrawn_b).unwrap()
            );
            let pool_account = utils::unpack_token_account(&pool_account.data).unwrap();
            assert_eq!(pool_account.amount, initial_pool - withdraw_amount);
            let admin_fee_key_a =
                utils::unpack_token_account(&accounts.admin_fee_a_account.data).unwrap();
            assert_eq!(admin_fee_key_a.amount, U256::to_u64(admin_fee_a).unwrap());
            let admin_fee_key_b =
                utils::unpack_token_account(&accounts.admin_fee_b_account.data).unwrap();
            assert_eq!(admin_fee_key_b.amount, U256::to_u64(admin_fee_b).unwrap());
        }
    }

    #[test]
    fn test_calc_receive_amount() {
        let user_key = pubkey_rand();
        let amp_factor = 85;
        let token_a_amount = FixedU64::new_from_u64(1000).unwrap();
        let token_b_amount = FixedU64::new_from_u64(1000).unwrap();
        let k = FixedU64::one()
            .checked_mul_floor(FixedU64::new(1))
            .unwrap()
            .checked_div_floor(FixedU64::new(10))
            .unwrap();
        let i = FixedU64::one();
        let is_open_twap = utils::TWAP_OPENED;
        let curve_mode = utils::CURVE_PMM;

        let swap_fees: Fees = Fees {
            admin_trade_fee_numerator: 1,
            admin_trade_fee_denominator: 1000,
            admin_withdraw_fee_numerator: 1,
            admin_withdraw_fee_denominator: 1000,
            trade_fee_numerator: 1,
            trade_fee_denominator: 2000,
            withdraw_fee_numerator: 1,
            withdraw_fee_denominator: 2000,
        };

        let swap_rewards = Rewards {
            trade_reward_numerator: 1,
            trade_reward_denominator: 1000,
            trade_reward_cap: 100,
        };

        let mut config_account = ConfigAccountInfo::new(amp_factor, swap_fees, swap_rewards);
        config_account.initialize().unwrap();

        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount.inner(),
            token_b_amount.inner(),
            k,
            i,
            is_open_twap,
            curve_mode,
        );

        let mut swap_direction = SWAP_DIRECTION_SELL_BASE;
        let pay_amount = FixedU64::new_from_u64(100).unwrap();
        let minimum_b_amount = pay_amount.checked_div_ceil(FixedU64::new(2)).unwrap();

        let swap_token_a_key = accounts.token_a_key;
        let swap_token_b_key = accounts.token_b_key;

        accounts.initialize_swap().unwrap();

        accounts
            .calc_receive_amount(
                &swap_token_a_key,
                &swap_token_b_key,
                pay_amount.inner(),
                minimum_b_amount.inner(),
                swap_direction,
                utils::CURVE_PMM,
            )
            .unwrap();

        let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();

        assert_eq!(swap_info.receive_amount.into_real_u64_ceil(), 100);

        swap_direction = SWAP_DIRECTION_SELL_QUOTE;

        accounts
            .calc_receive_amount(
                &swap_token_a_key,
                &swap_token_b_key,
                pay_amount.inner(),
                minimum_b_amount.inner(),
                swap_direction,
                utils::CURVE_AMM,
            )
            .unwrap();

        let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
        assert_eq!(swap_info.receive_amount.into_real_u64_ceil(), 91);
    }

    #[test]
    fn test_sell_buy() {
        let user_key = pubkey_rand();
        let swapper_key = pubkey_rand();
        let amp_factor = 85;
        let token_a_amount = FixedU64::new_from_u64(1000).unwrap();
        let token_b_amount = FixedU64::new_from_u64(1000).unwrap();
        let k = FixedU64::one()
            .checked_mul_floor(FixedU64::new(1))
            .unwrap()
            .checked_div_floor(FixedU64::new(10))
            .unwrap();
        let i = FixedU64::one();
        let is_open_twap = utils::TWAP_OPENED;
        let curve_mode = utils::CURVE_PMM;

        let swap_fees: Fees = Fees {
            admin_trade_fee_numerator: 1,
            admin_trade_fee_denominator: 1000,
            admin_withdraw_fee_numerator: 1,
            admin_withdraw_fee_denominator: 1000,
            trade_fee_numerator: 1,
            trade_fee_denominator: 2000,
            withdraw_fee_numerator: 1,
            withdraw_fee_denominator: 2000,
        };

        let swap_rewards = Rewards {
            trade_reward_numerator: 1,
            trade_reward_denominator: 1000,
            trade_reward_cap: 100,
        };

        let mut config_account = ConfigAccountInfo::new(amp_factor, swap_fees, swap_rewards);
        config_account.initialize().unwrap();

        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount.inner(),
            token_b_amount.inner(),
            k,
            i,
            is_open_twap,
            curve_mode,
        );

        let initial_a = token_a_amount.checked_div_ceil(FixedU64::new(2)).unwrap();
        let initial_b = token_b_amount.checked_div_ceil(FixedU64::new(2)).unwrap();
        let mut swap_direction = SWAP_DIRECTION_SELL_BASE;
        let pay_amount = FixedU64::new_from_u64(100).unwrap();
        let minimum_b_amount = pay_amount.checked_div_ceil(FixedU64::new(2)).unwrap();

        let swap_token_a_key = accounts.token_a_key;
        let swap_token_b_key = accounts.token_b_key;

        accounts.initialize_swap().unwrap();
        let initial_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();

        let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
        let token_a_amount = swap_token_a.amount;

        let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
        let token_b_amount = swap_token_b.amount;

        let swap_token_admin_fee_a =
            utils::unpack_token_account(&accounts.admin_fee_a_account.data).unwrap();
        let token_admin_fee_a_amount = swap_token_admin_fee_a.amount;

        let swap_token_admin_fee_b =
            utils::unpack_token_account(&accounts.admin_fee_b_account.data).unwrap();
        let token_admin_fee_b_amount = swap_token_admin_fee_b.amount;

        let swap_reward_token =
            utils::unpack_token_account(&accounts.deltafi_token_account.data).unwrap();
        let deltafi_reward_amount = swap_reward_token.amount;

        assert_eq!(token_a_amount, 1000000000);
        assert_eq!(token_b_amount, 1000000000);
        // assert_eq!(token_admin_amount, 1000);
        assert_eq!(token_admin_fee_a_amount, 0);
        assert_eq!(token_admin_fee_b_amount, 0);
        assert_eq!(deltafi_reward_amount, 0);
        assert_eq!(initial_info.pmm_state.b_0.inner(), 1000000000);
        assert_eq!(initial_info.pmm_state.q_0.inner(), 1000000000);
        assert_eq!(initial_info.pmm_state.b.inner(), 1000000000);
        assert_eq!(initial_info.pmm_state.q.inner(), 1000000000);

        let (
            token_a_key,
            mut token_a_account,
            token_b_key,
            mut token_b_account,
            _pool_key,
            _pool_account,
        ) = accounts.setup_token_accounts(
            &user_key,
            &swapper_key,
            initial_a.inner(),
            initial_b.inner(),
            0,
        );

        accounts
            .swap(
                &swapper_key,
                &token_a_key,
                &mut token_a_account,
                &swap_token_a_key,
                &swap_token_b_key,
                &token_b_key,
                &mut token_b_account,
                pay_amount.inner(),
                minimum_b_amount.inner(),
                swap_direction,
                curve_mode,
            )
            .unwrap();
        let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
        let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
        let token_a_amount = swap_token_a.amount;

        let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
        let token_b_amount = swap_token_b.amount;

        let swap_token_admin_fee_a =
            utils::unpack_token_account(&accounts.admin_fee_a_account.data).unwrap();
        let token_admin_fee_a_amount = swap_token_admin_fee_a.amount;

        let swap_token_admin_fee_b =
            utils::unpack_token_account(&accounts.admin_fee_b_account.data).unwrap();
        let token_admin_fee_b_amount = swap_token_admin_fee_b.amount;

        let swap_reward_token =
            utils::unpack_token_account(&accounts.deltafi_token_account.data).unwrap();
        let deltafi_reward_amount = swap_reward_token.amount;

        let user_token_a = utils::unpack_token_account(&token_a_account.data).unwrap();
        let user_token_a_amount = user_token_a.amount;

        let user_token_b = utils::unpack_token_account(&token_b_account.data).unwrap();
        let user_token_b_amount = user_token_b.amount;

        assert_eq!(
            user_token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            400
        );
        assert_eq!(
            user_token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            590
        );
        assert_eq!(
            token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            1100
        );
        assert_eq!(token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(), 909);
        assert_eq!(token_admin_fee_a_amount, 0);
        assert_eq!(token_admin_fee_b_amount, 45);
        assert_eq!(deltafi_reward_amount, 10000);
        assert_eq!(swap_info.pmm_state.b_0.into_real_u64_ceil(), 1000);
        assert_eq!(swap_info.pmm_state.q_0.into_real_u64_ceil(), 1000);
        assert_eq!(swap_info.pmm_state.b.into_real_u64_ceil(), 1100);
        assert_eq!(swap_info.pmm_state.q.into_real_u64_ceil(), 910);

        swap_direction = SWAP_DIRECTION_SELL_QUOTE;

        accounts
            .swap(
                &swapper_key,
                &token_a_key,
                &mut token_a_account,
                &swap_token_a_key,
                &swap_token_b_key,
                &token_b_key,
                &mut token_b_account,
                pay_amount.inner(),
                minimum_b_amount.inner(),
                swap_direction,
                curve_mode,
            )
            .unwrap();
        let swap_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();
        let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
        let token_a_amount = swap_token_a.amount;

        let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
        let token_b_amount = swap_token_b.amount;

        let swap_token_admin_fee_a =
            utils::unpack_token_account(&accounts.admin_fee_a_account.data).unwrap();
        let token_admin_fee_a_amount = swap_token_admin_fee_a.amount;

        let swap_token_admin_fee_b =
            utils::unpack_token_account(&accounts.admin_fee_b_account.data).unwrap();
        let token_admin_fee_b_amount = swap_token_admin_fee_b.amount;

        let swap_reward_token =
            utils::unpack_token_account(&accounts.deltafi_token_account.data).unwrap();
        let deltafi_reward_amount = swap_reward_token.amount;

        let user_token_a = utils::unpack_token_account(&token_a_account.data).unwrap();
        let user_token_a_amount = user_token_a.amount;

        let user_token_b = utils::unpack_token_account(&token_b_account.data).unwrap();
        let user_token_b_amount = user_token_b.amount;

        assert_eq!(
            user_token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            504
        );
        assert_eq!(
            user_token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            490
        );
        assert_eq!(token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(), 995);
        assert_eq!(
            token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            1009
        );
        assert_eq!(token_admin_fee_a_amount, 52);
        assert_eq!(token_admin_fee_b_amount, 45);
        assert_eq!(deltafi_reward_amount, 19891);
        assert_eq!(swap_info.pmm_state.b_0.into_real_u64_ceil(), 1000);
        assert_eq!(swap_info.pmm_state.q_0.into_real_u64_ceil(), 1005);
        assert_eq!(swap_info.pmm_state.b.into_real_u64_ceil(), 996);
        assert_eq!(swap_info.pmm_state.q.into_real_u64_ceil(), 1010);
    }

    #[test]
    fn test_swap() {
        let user_key = pubkey_rand();
        let swapper_key = pubkey_rand();
        let amp_factor = 85;
        let token_a_amount = 100;
        let token_b_amount = 10000;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount,
            token_b_amount,
            default_k(),
            default_i(),
            utils::TWAP_OPENED,
            utils::CURVE_PMM,
        );
        let initial_a = token_a_amount;
        let initial_b = token_b_amount;
        let minimum_b_amount = initial_b / 20;
        let swap_direction = SWAP_DIRECTION_SELL_BASE;
        let curve_mode = CURVE_PMM;

        let swap_token_a_key = accounts.token_a_key;
        let swap_token_b_key = accounts.token_b_key;

        // swap not initialized
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.swap(
                    &swapper_key,
                    &token_a_key,
                    &mut token_a_account,
                    &swap_token_a_key,
                    &swap_token_b_key,
                    &token_b_key,
                    &mut token_b_account,
                    initial_a,
                    minimum_b_amount,
                    swap_direction,
                    curve_mode,
                )
            );
        }

        accounts.initialize_swap().unwrap();

        // wrong nonce
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            let old_authority = accounts.authority_key;
            let (bad_authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.swap_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = bad_authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.swap(
                    &swapper_key,
                    &token_a_key,
                    &mut token_a_account,
                    &swap_token_a_key,
                    &swap_token_b_key,
                    &token_b_key,
                    &mut token_b_account,
                    initial_a,
                    minimum_b_amount,
                    swap_direction,
                    curve_mode,
                )
            );
            accounts.authority_key = old_authority;
        }

        // wrong token program id
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            let wrong_program_id = pubkey_rand();
            assert_eq!(
                Err(ProgramError::IncorrectProgramId),
                do_process_instruction(
                    swap(
                        &SWAP_PROGRAM_ID,
                        &wrong_program_id,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &accounts.admin_fee_b_key,
                        &accounts.pyth_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
                        curve_mode,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut token_a_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut token_b_account,
                        &mut accounts.deltafi_token_account,
                        &mut accounts.deltafi_mint_account,
                        &mut accounts.admin_fee_b_account,
                        &mut accounts.pyth_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                ),
            );
        }

        // not enough token a to swap
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(ProgramError::InsufficientFunds.into()),
                accounts.swap(
                    &swapper_key,
                    &token_a_key,
                    &mut token_a_account,
                    &swap_token_a_key,
                    &swap_token_b_key,
                    &token_b_key,
                    &mut token_b_account,
                    initial_a * 2,
                    minimum_b_amount * 2,
                    swap_direction,
                    curve_mode,
                )
            );
        }

        // wrong swap token A / B accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                do_process_instruction(
                    swap(
                        &SWAP_PROGRAM_ID,
                        &spl_token::id(),
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &token_a_key,
                        &token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &accounts.admin_fee_b_key,
                        &accounts.pyth_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
                        curve_mode,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut token_a_account.clone(),
                        &mut token_a_account,
                        &mut token_b_account.clone(),
                        &mut token_b_account,
                        &mut accounts.deltafi_token_account,
                        &mut accounts.deltafi_mint_account,
                        &mut accounts.admin_fee_b_account,
                        &mut accounts.pyth_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                ),
            );
        }

        // wrong admin account
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                wrong_admin_key,
                mut wrong_admin_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(SwapError::InvalidAdmin.into()),
                do_process_instruction(
                    swap(
                        &SWAP_PROGRAM_ID,
                        &spl_token::id(),
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &wrong_admin_key,
                        &accounts.pyth_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
                        curve_mode,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut token_a_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut token_b_account,
                        &mut accounts.deltafi_token_account,
                        &mut accounts.deltafi_mint_account,
                        &mut wrong_admin_account,
                        &mut accounts.pyth_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                ),
            );
        }

        // wrong user token A / B accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.swap(
                    &swapper_key,
                    &token_b_key,
                    &mut token_b_account,
                    &swap_token_a_key,
                    &swap_token_b_key,
                    &token_a_key,
                    &mut token_a_account,
                    initial_a,
                    minimum_b_amount,
                    swap_direction,
                    curve_mode,
                )
            );
        }

        // swap from a to a
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.swap(
                    &swapper_key,
                    &token_a_key,
                    &mut token_a_account.clone(),
                    &swap_token_a_key,
                    &swap_token_a_key,
                    &token_a_key,
                    &mut token_a_account,
                    initial_a,
                    minimum_b_amount,
                    swap_direction,
                    curve_mode,
                )
            );
        }

        // no approval
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(TokenError::OwnerMismatch.into()),
                do_process_instruction(
                    swap(
                        &SWAP_PROGRAM_ID,
                        &spl_token::id(),
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &accounts.admin_fee_b_key,
                        &accounts.pyth_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
                        curve_mode,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut token_a_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut accounts.admin_fee_b_account,
                        &mut accounts.deltafi_token_account,
                        &mut accounts.deltafi_mint_account,
                        &mut token_b_account,
                        &mut accounts.pyth_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                ),
            );
        }

        // slippage exceeeded: minimum out amount too high
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(SwapError::ExceededSlippage.into()),
                accounts.swap(
                    &swapper_key,
                    &token_a_key,
                    &mut token_a_account,
                    &swap_token_a_key,
                    &swap_token_b_key,
                    &token_b_key,
                    &mut token_b_account,
                    initial_a,
                    minimum_b_amount * 10,
                    swap_direction,
                    curve_mode,
                )
            );
        }

        // Pool is paused
        {
            let (
                token_a_key,
                mut token_a_account,
                token_b_key,
                mut token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(&user_key, &swapper_key, initial_a, initial_b, 0);
            // Pause pool
            accounts.pause().unwrap();

            assert_eq!(
                Err(SwapError::IsPaused.into()),
                accounts.swap(
                    &swapper_key,
                    &token_a_key,
                    &mut token_a_account,
                    &swap_token_a_key,
                    &swap_token_b_key,
                    &token_b_key,
                    &mut token_b_account,
                    initial_a,
                    minimum_b_amount,
                    swap_direction,
                    curve_mode,
                )
            );
        }
    }

    #[test]
    fn test_withdraw_one() {
        let user_key = pubkey_rand();
        let amp_factor = MIN_AMP;
        let token_a_amount = 1000;
        let token_b_amount = 1000;
        let mut config_account =
            ConfigAccountInfo::new(amp_factor, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS);
        config_account.initialize().unwrap();
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            &config_account,
            token_a_amount,
            token_b_amount,
            default_k(),
            default_i(),
            utils::TWAP_OPENED,
            utils::CURVE_PMM,
        );
        let withdrawer_key = pubkey_rand();
        let initial_a = token_a_amount / 10;
        let initial_b = token_b_amount / 10;
        let initial_pool = initial_a + initial_b;
        // Withdraw entire pool share
        let withdraw_amount = initial_pool;
        let minimum_amount = 0;

        // swap not initialized
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &withdrawer_key, initial_a, initial_b, 0);
            assert_eq!(
                Err(ProgramError::UninitializedAccount),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );
        }

        accounts.initialize_swap().unwrap();

        // wrong nonce for authority_key
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &withdrawer_key, initial_a, initial_b, 0);
            let old_authority = accounts.authority_key;
            let (bad_authority_key, _nonce) = Pubkey::find_program_address(
                &[&accounts.swap_key.to_bytes()[..]],
                &spl_token::id(),
            );
            accounts.authority_key = bad_authority_key;
            assert_eq!(
                Err(SwapError::InvalidProgramAddress.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );
            accounts.authority_key = old_authority;
        }

        // not enough pool tokens
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount * 100,
                    minimum_amount,
                )
            );
        }

        // same swap / quote accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );

            let old_token_b_key = accounts.token_b_key;
            let old_token_b_account = accounts.token_b_account;
            accounts.token_b_key = accounts.token_a_key;
            accounts.token_b_account = accounts.token_a_account.clone();

            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );

            accounts.token_b_key = old_token_b_key;
            accounts.token_b_account = old_token_b_account;
        }

        // foreign swap / quote accounts
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            let foreign_authority = pubkey_rand();
            let (foreign_mint_key, mut foreign_mint_account) = create_mint(
                &spl_token::id(),
                &foreign_authority,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let (foreign_token_key, foreign_token_account) = mint_token(
                &spl_token::id(),
                &foreign_mint_key,
                &mut foreign_mint_account,
                &foreign_authority,
                &pubkey_rand(),
                0,
            );

            let old_token_a_key = accounts.token_a_key;
            let old_token_a_account = accounts.token_a_account;
            accounts.token_a_key = foreign_token_key;
            accounts.token_a_account = foreign_token_account.clone();

            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );

            accounts.token_a_key = old_token_a_key;
            accounts.token_a_account = old_token_a_account;

            let old_token_b_key = accounts.token_b_key;
            let old_token_b_account = accounts.token_b_account;
            accounts.token_b_key = foreign_token_key;
            accounts.token_b_account = foreign_token_account;

            assert_eq!(
                Err(SwapError::IncorrectSwapAccount.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );

            accounts.token_b_key = old_token_b_key;
            accounts.token_b_account = old_token_b_account;
        }

        // wrong pool token account
        {
            let (
                token_a_key,
                mut token_a_account,
                wrong_token_b_key,
                mut wrong_token_b_account,
                _pool_key,
                _pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                withdraw_amount,
                withdraw_amount,
            );
            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &wrong_token_b_key,
                    &mut wrong_token_b_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );
        }

        // no approval
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(&user_key, &withdrawer_key, 0, 0, withdraw_amount);
            assert_eq!(
                Err(TokenError::OwnerMismatch.into()),
                do_process_instruction(
                    withdraw_one(
                        &SWAP_PROGRAM_ID,
                        &spl_token::id(),
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_a_key,
                        &accounts.admin_fee_a_key,
                        withdraw_amount,
                        minimum_amount,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut accounts.pool_mint_account,
                        &mut pool_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut token_a_account,
                        &mut accounts.admin_fee_a_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                )
            );
        }

        // wrong token program id
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );
            let wrong_key = pubkey_rand();
            assert_eq!(
                Err(ProgramError::IncorrectProgramId),
                do_process_instruction(
                    withdraw_one(
                        &SWAP_PROGRAM_ID,
                        &wrong_key,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_a_key,
                        &accounts.admin_fee_a_key,
                        withdraw_amount,
                        minimum_amount,
                    )
                    .unwrap(),
                    vec![
                        &mut accounts.swap_account,
                        &mut Account::default(),
                        &mut accounts.pool_mint_account,
                        &mut pool_account,
                        &mut accounts.token_a_account,
                        &mut accounts.token_b_account,
                        &mut token_a_account,
                        &mut accounts.admin_fee_a_account,
                        &mut Account::default(),
                        &mut clock_account(ZERO_TS),
                    ],
                )
            );
        }

        // wrong mint
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );
            let (pool_mint_key, pool_mint_account) = create_mint(
                &spl_token::id(),
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let old_pool_key = accounts.pool_mint_key;
            let old_pool_account = accounts.pool_mint_account;
            accounts.pool_mint_key = pool_mint_key;
            accounts.pool_mint_account = pool_mint_account;

            assert_eq!(
                Err(SwapError::IncorrectMint.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );

            accounts.pool_mint_key = old_pool_key;
            accounts.pool_mint_account = old_pool_account;
        }

        // wrong destination account
        {
            let (
                _token_a_key,
                _token_a_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );

            assert_eq!(
                Err(TokenError::MintMismatch.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );
        }

        // wrong admin account
        {
            let (
                wrong_admin_key,
                wrong_admin_account,
                token_b_key,
                mut token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                withdraw_amount,
            );

            let old_admin_a_key = accounts.admin_fee_a_key;
            let old_admin_a_account = accounts.admin_fee_a_account;
            accounts.admin_fee_a_key = wrong_admin_key;
            accounts.admin_fee_a_account = wrong_admin_account;

            assert_eq!(
                Err(SwapError::InvalidAdmin.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_b_key,
                    &mut token_b_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );

            accounts.admin_fee_a_key = old_admin_a_key;
            accounts.admin_fee_a_account = old_admin_a_account;
        }

        // slippage exceeeded
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );

            let high_minimum_amount = 100000;
            assert_eq!(
                Err(SwapError::ExceededSlippage.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    high_minimum_amount,
                )
            );
        }

        // correct withdraw
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );

            let old_swap_token_a =
                utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
            let old_swap_token_b =
                utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
            let old_pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();

            let invariant = StableSwap::new(
                accounts.initial_amp_factor,
                accounts.target_amp_factor,
                ZERO_TS,
                ZERO_TS,
                ZERO_TS,
            );
            let (withdraw_one_amount_before_fees, withdraw_one_trade_fee) = invariant
                .compute_withdraw_one(
                    withdraw_amount.into(),
                    old_pool_mint.supply.into(),
                    old_swap_token_a.amount.into(),
                    old_swap_token_b.amount.into(),
                    &DEFAULT_TEST_FEES,
                )
                .unwrap();
            let withdraw_one_withdraw_fee = DEFAULT_TEST_FEES
                .withdraw_fee_256(withdraw_one_amount_before_fees)
                .unwrap();
            let expected_withdraw_one_amount =
                withdraw_one_amount_before_fees - withdraw_one_withdraw_fee;
            let expected_admin_fee = U256::to_u64(
                DEFAULT_TEST_FEES
                    .admin_trade_fee_256(withdraw_one_trade_fee)
                    .unwrap()
                    + DEFAULT_TEST_FEES
                        .admin_withdraw_fee_256(withdraw_one_withdraw_fee)
                        .unwrap(),
            )
            .unwrap();

            accounts
                .withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
                .unwrap();

            let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
            assert_eq!(
                old_swap_token_a.amount - swap_token_a.amount - expected_admin_fee,
                U256::to_u64(expected_withdraw_one_amount).unwrap()
            );
            let admin_fee_key_a =
                utils::unpack_token_account(&accounts.admin_fee_a_account.data).unwrap();
            assert_eq!(admin_fee_key_a.amount, expected_admin_fee);
            let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
            assert_eq!(swap_token_b.amount, old_swap_token_b.amount);
            let pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();
            assert_eq!(pool_mint.supply, old_pool_mint.supply - withdraw_amount);
        }

        // pool is paused
        {
            let (
                token_a_key,
                mut token_a_account,
                _token_b_key,
                _token_b_account,
                pool_key,
                mut pool_account,
            ) = accounts.setup_token_accounts(
                &user_key,
                &withdrawer_key,
                initial_a,
                initial_b,
                initial_pool,
            );
            // pause pool
            accounts.pause().unwrap();

            assert_eq!(
                Err(SwapError::IsPaused.into()),
                accounts.withdraw_one(
                    &withdrawer_key,
                    &pool_key,
                    &mut pool_account,
                    &token_a_key,
                    &mut token_a_account,
                    withdraw_amount,
                    minimum_amount,
                )
            );
        }
    }
}
