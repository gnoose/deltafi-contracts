//! Program state processor

#![allow(clippy::too_many_arguments)]

use std::time::{SystemTime, UNIX_EPOCH};

use num_traits::FromPrimitive;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::{PrintProgramError, ProgramError},
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};
use spl_token::state::Mint;

use crate::{
    admin::process_admin_instruction,
    bn::{FixedU256, U256},
    curve::{StableSwap, MAX_AMP, MIN_AMP, ZERO_TS},
    error::SwapError,
    fees::Fees,
    instruction::{
        AdminInstruction, DepositData, InitializeData, SwapData, SwapInstruction, WithdrawData,
        WithdrawOneData,
    },
    math2::{get_buy_shares, get_deposit_adjustment_amount},
    oracle::Oracle,
    pool_converter::PoolTokenConverter,
    rewards::Rewards,
    state::SwapInfo,
    utils,
    utils::TWAP_OPENED,
    v2curve::{adjusted_target, get_mid_price, sell_base_token, sell_quote_token, PMMState},
};

/// Program state handler. (and general curve params)
pub struct Processor {}

impl Processor {
    /// Unpacks a spl_token `Mint`.
    pub fn unpack_mint(data: &[u8]) -> Result<Mint, SwapError> {
        Mint::unpack(data).map_err(|_| SwapError::ExpectedMint)
    }

    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn<'a>(
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

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
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

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
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

    /// Processes an [Initialize](enum.Instruction.html).
    #[allow(clippy::too_many_arguments)]
    pub fn process_initialize(
        program_id: &Pubkey,
        nonce: u8,
        amp_factor: u64,
        fees: Fees,
        rewards: Rewards,
        k_v: u64,
        i_v: u64,
        is_open_twap: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let admin_key_info = next_account_info(account_info_iter)?;
        let admin_fee_a_info = next_account_info(account_info_iter)?;
        let admin_fee_b_info = next_account_info(account_info_iter)?;
        let token_a_mint_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_mint_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?; // Destination account to mint LP tokens to
        let deltafi_mint_info = next_account_info(account_info_iter)?;
        let deltafi_token_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let k = FixedU256::new_from_fixed_u64(k_v)?;
        let i = FixedU256::new_from_fixed_u64(i_v)?;

        if !(MIN_AMP..=MAX_AMP).contains(&amp_factor) {
            return Err(SwapError::InvalidInput.into());
        }

        let token_swap = SwapInfo::unpack_unchecked(&swap_info.data.borrow())?;
        if token_swap.is_initialized {
            return Err(SwapError::AlreadyInUse.into());
        }
        if *authority_info.key != utils::authority_id(program_id, swap_info.key, nonce)? {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        let destination = utils::unpack_token_account(&destination_info.data.borrow())?;
        let token_a = utils::unpack_token_account(&token_a_info.data.borrow())?;
        let token_b = utils::unpack_token_account(&token_b_info.data.borrow())?;
        let deltafi_token = utils::unpack_token_account(&deltafi_token_info.data.borrow())?;
        if *authority_info.key != token_a.owner {
            return Err(SwapError::InvalidOwner.into());
        }
        if *authority_info.key != token_b.owner {
            return Err(SwapError::InvalidOwner.into());
        }
        if *authority_info.key == destination.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if *authority_info.key == deltafi_token.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if token_a.mint == token_b.mint {
            return Err(SwapError::RepeatedMint.into());
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
        if token_a.mint != *token_a_mint_info.key {
            return Err(SwapError::IncorrectMint.into());
        }
        if token_b.mint != *token_b_mint_info.key {
            return Err(SwapError::IncorrectMint.into());
        }
        if token_a.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }
        if token_b.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }
        if deltafi_token.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }
        let pool_mint = Self::unpack_mint(&pool_mint_info.data.borrow())?;
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
        let token_a_mint = Self::unpack_mint(&token_a_mint_info.data.borrow())?;
        let token_b_mint = Self::unpack_mint(&token_b_mint_info.data.borrow())?;
        if token_a_mint.decimals != token_b_mint.decimals {
            return Err(SwapError::MismatchedDecimals.into());
        }
        if pool_mint.decimals != token_a_mint.decimals {
            return Err(SwapError::MismatchedDecimals.into());
        }
        let deltafi_mint = Self::unpack_mint(&deltafi_mint_info.data.borrow())?;
        if deltafi_mint.mint_authority.is_some()
            && *authority_info.key != deltafi_mint.mint_authority.unwrap()
        {
            return Err(SwapError::InvalidOwner.into());
        }
        if deltafi_mint.freeze_authority.is_some() {
            return Err(SwapError::InvalidFreezeAuthority.into());
        }
        if deltafi_mint.decimals != token_a_mint.decimals {
            return Err(SwapError::MismatchedDecimals.into());
        }
        let admin_fee_key_a = utils::unpack_token_account(&admin_fee_a_info.data.borrow())?;
        let admin_fee_key_b = utils::unpack_token_account(&admin_fee_b_info.data.borrow())?;
        if token_a.mint != admin_fee_key_a.mint {
            return Err(SwapError::InvalidAdmin.into());
        }
        if token_b.mint != admin_fee_key_b.mint {
            return Err(SwapError::InvalidAdmin.into());
        }

        // calc mint amount with pmm - lp token amount

        let base_balance = FixedU256::new_from_fixed_u64(token_a.amount)?;
        let quote_balance = FixedU256::new_from_fixed_u64(token_b.amount)?;
        let base_reserve = FixedU256::zero();
        let quote_reserve = FixedU256::zero();
        let total_supply = FixedU256::new_from_fixed_u64(pool_mint.supply)?;
        let base_target = FixedU256::zero();
        let quote_target = FixedU256::zero();
        let block_timestamp_last: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let (mint_amount, new_base_target, new_quote_target, new_base_reserve, new_quote_reserve) =
            get_buy_shares(
                base_balance,
                quote_balance,
                base_reserve,
                quote_reserve,
                base_target,
                quote_target,
                total_supply,
                i,
            )?;

        Self::token_mint_to(
            swap_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            nonce,
            mint_amount.inner_u64()?,
        )?;
        let new_block_timestamp_last: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut base_price_cumulative_last = FixedU256::zero();
        if is_open_twap == TWAP_OPENED {
            let time_elapsed = new_block_timestamp_last - block_timestamp_last;
            if time_elapsed > 0
                && new_base_reserve.inner_u64()? != 0
                && new_quote_reserve.inner_u64()? != 0
            {
                let state = PMMState::new(
                    i,
                    k,
                    new_base_reserve,
                    new_quote_reserve,
                    new_base_target,
                    new_quote_target,
                    token_swap.r,
                );

                base_price_cumulative_last = base_price_cumulative_last.checked_add(
                    get_mid_price(state)?
                        .checked_mul_floor(FixedU256::new_from_u64(time_elapsed as u64)?)?,
                )?;
            }
        }

        let obj = SwapInfo {
            is_initialized: true,
            is_paused: false,
            nonce,
            initial_amp_factor: amp_factor,
            target_amp_factor: amp_factor,
            start_ramp_ts: ZERO_TS,
            stop_ramp_ts: ZERO_TS,
            future_admin_deadline: ZERO_TS,
            future_admin_key: Pubkey::default(),
            admin_key: *admin_key_info.key,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            deltafi_token: *deltafi_token_info.key,
            pool_mint: *pool_mint_info.key,
            token_a_mint: token_a.mint,
            token_b_mint: token_b.mint,
            deltafi_mint: deltafi_token.mint,
            admin_fee_key_a: *admin_fee_a_info.key,
            admin_fee_key_b: *admin_fee_b_info.key,
            fees,
            rewards,
            oracle: Oracle::new(*token_a_info.key, *token_b_info.key),
            k,
            i,
            r: token_swap.r,
            base_target: new_base_target,
            quote_target: new_quote_target,
            base_reserve: new_base_reserve,
            quote_reserve: new_quote_reserve,
            is_open_twap,
            block_timestamp_last: new_block_timestamp_last,
            base_price_cumulative_last,
        };
        SwapInfo::pack(obj, &mut swap_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes an [Swap](enum.Instruction.html).
    pub fn process_swap(
        program_id: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        swap_direction: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        msg!("swap_direction ................ {}", swap_direction);
        match swap_direction {
            utils::SWAP_DIRECTION_SELL_BASE => {
                Self::sell_base(program_id, amount_in, minimum_amount_out, accounts)
            }
            utils::SWAP_DIRECTION_SELL_QUOTE => {
                Self::sell_quote(program_id, amount_in, minimum_amount_out, accounts)
            }
            _ => Ok(()),
        }
    }

    /// processor for sell base
    pub fn sell_base(
        program_id: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let swap_source_info = next_account_info(account_info_iter)?;
        let swap_destination_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let reward_token_info = next_account_info(account_info_iter)?;
        let reward_mint_info = next_account_info(account_info_iter)?;
        let admin_destination_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let _clock_sysvar_info = next_account_info(account_info_iter)?;

        let token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
        if token_swap.is_paused {
            return Err(SwapError::IsPaused.into());
        }
        if *authority_info.key != utils::authority_id(program_id, swap_info.key, token_swap.nonce)?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if !(*swap_source_info.key == token_swap.token_a
            || *swap_source_info.key == token_swap.token_b)
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if !(*swap_destination_info.key == token_swap.token_a
            || *swap_destination_info.key == token_swap.token_b)
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
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
        if *swap_source_info.key == *swap_destination_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        if *reward_mint_info.key != token_swap.deltafi_mint {
            return Err(SwapError::IncorrectMint.into());
        }
        if *reward_token_info.key != token_swap.deltafi_token {
            return Err(SwapError::IncorrectRewardAccount.into());
        }

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token_a = utils::unpack_token_account(&swap_source_info.data.borrow())?;
        let token_b = utils::unpack_token_account(&swap_destination_info.data.borrow())?;

        // oracle update

        let (price0_cumulative, price1_cumulative, block_timestamp) =
            token_swap.oracle.current_cumulative_price(
                U256::from(token_a.amount),
                U256::from(token_b.amount),
                current_timestamp,
            );
        let mut swap = token_swap;

        swap.oracle
            .update(price0_cumulative, price1_cumulative, block_timestamp);

        // calculate swap amount with pmm

        let base_balance = FixedU256::new_from_fixed_u64(token_a.amount)?;
        let quote_balance = FixedU256::new_from_fixed_u64(token_b.amount)?;
        let pay_base_amount = FixedU256::new_from_fixed_u64(amount_in)?;

        if base_balance.inner_u64()? < pay_base_amount.inner_u64()? {
            return Err(ProgramError::InsufficientFunds);
        }

        let base_reserve = token_swap.base_reserve;
        let quote_reserve = token_swap.quote_reserve;
        let base_target = token_swap.base_target;
        let quote_target = token_swap.quote_target;

        let mut state = PMMState::new(
            token_swap.i,
            token_swap.k,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            token_swap.r,
        );
        adjusted_target(&mut state)?;

        let (receive_quote_amount, new_r) = sell_base_token(state, pay_base_amount)?;

        let fees = &token_swap.fees;
        let dy_fee;
        match fees.trade_fee(receive_quote_amount.inner()) {
            Some(v) => {
                dy_fee = FixedU256::new_from_fixed_u256(v)?;
            }
            None => {
                return Err(ProgramError::InvalidArgument);
            }
        }
        let admin_fee;
        match fees.admin_trade_fee(dy_fee.into_u256_ceil()) {
            Some(v) => {
                admin_fee = FixedU256::new_from_fixed_u256(v)?;
            }
            None => {
                return Err(ProgramError::InvalidArgument);
            }
        }
        let dy_swap_amount = receive_quote_amount.checked_sub(dy_fee)?;
        let new_base_target = state.b_0;
        let new_quote_target = quote_target;

        if dy_swap_amount.inner_u64()? < minimum_amount_out {
            return Err(SwapError::ExceededSlippage.into());
        }

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            swap_source_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            amount_in,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            swap_destination_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            dy_swap_amount.inner_u64()?,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            swap_destination_info.clone(),
            admin_destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            admin_fee.inner_u64()?,
        )?;

        let new_base_reserve = base_balance.checked_add(pay_base_amount)?;
        let new_quote_reserve = quote_balance.checked_sub(dy_swap_amount)?;

        let block_timestamp_last: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut base_price_cumulative_last = token_swap.base_price_cumulative_last;
        if token_swap.is_open_twap == TWAP_OPENED {
            let time_elapsed = block_timestamp_last - token_swap.block_timestamp_last;
            if time_elapsed > 0
                && new_base_reserve.inner_u64()? != 0
                && new_quote_reserve.inner_u64()? != 0
            {
                let state = PMMState::new(
                    token_swap.i,
                    token_swap.k,
                    new_base_reserve,
                    new_quote_reserve,
                    new_base_target,
                    new_quote_target,
                    token_swap.r,
                );

                base_price_cumulative_last = base_price_cumulative_last.checked_add(
                    get_mid_price(state)?
                        .checked_mul_floor(FixedU256::new_from_u64(time_elapsed as u64)?)?,
                )?;
            }
        }

        let obj = SwapInfo {
            is_initialized: token_swap.is_initialized,
            is_paused: token_swap.is_paused,
            nonce: token_swap.nonce,
            initial_amp_factor: token_swap.initial_amp_factor,
            target_amp_factor: token_swap.target_amp_factor,
            start_ramp_ts: token_swap.start_ramp_ts,
            stop_ramp_ts: token_swap.stop_ramp_ts,
            future_admin_deadline: token_swap.future_admin_deadline,
            future_admin_key: token_swap.future_admin_key,
            admin_key: token_swap.admin_key,
            token_a: token_swap.token_a,
            token_b: token_swap.token_b,
            pool_mint: token_swap.pool_mint,
            token_a_mint: token_swap.token_a_mint,
            token_b_mint: token_swap.token_b_mint,
            admin_fee_key_a: token_swap.admin_fee_key_a,
            admin_fee_key_b: token_swap.admin_fee_key_b,
            deltafi_token: token_swap.deltafi_token,
            deltafi_mint: token_swap.deltafi_mint,
            fees: token_swap.fees,
            oracle: token_swap.oracle,
            rewards: token_swap.rewards,
            k: token_swap.k,
            i: token_swap.i,
            r: new_r,
            base_target: new_base_target,
            quote_target: new_quote_target,
            base_reserve: new_base_reserve,
            quote_reserve: new_quote_reserve,
            is_open_twap: token_swap.is_open_twap,
            block_timestamp_last,
            base_price_cumulative_last,
        };
        SwapInfo::pack(obj, &mut swap_info.data.borrow_mut())?;

        Ok(())
    }

    /// processor for sell quote
    pub fn sell_quote(
        program_id: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let swap_source_info = next_account_info(account_info_iter)?;
        let swap_destination_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let reward_token_info = next_account_info(account_info_iter)?;
        let reward_mint_info = next_account_info(account_info_iter)?;
        let admin_destination_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let _clock_sysvar_info = next_account_info(account_info_iter)?;

        let token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
        if token_swap.is_paused {
            return Err(SwapError::IsPaused.into());
        }
        if *authority_info.key != utils::authority_id(program_id, swap_info.key, token_swap.nonce)?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if !(*swap_source_info.key == token_swap.token_a
            || *swap_source_info.key == token_swap.token_b)
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if !(*swap_destination_info.key == token_swap.token_a
            || *swap_destination_info.key == token_swap.token_b)
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
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
        if *swap_source_info.key == *swap_destination_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        if *reward_mint_info.key != token_swap.deltafi_mint {
            return Err(SwapError::IncorrectMint.into());
        }
        if *reward_token_info.key != token_swap.deltafi_token {
            return Err(SwapError::IncorrectRewardAccount.into());
        }

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token_a = utils::unpack_token_account(&swap_source_info.data.borrow())?;
        let token_b = utils::unpack_token_account(&swap_destination_info.data.borrow())?;

        // oracle update

        let (price0_cumulative, price1_cumulative, block_timestamp) =
            token_swap.oracle.current_cumulative_price(
                U256::from(token_a.amount),
                U256::from(token_b.amount),
                current_timestamp,
            );
        let mut swap = token_swap;

        swap.oracle
            .update(price0_cumulative, price1_cumulative, block_timestamp);

        // calculate swap amount with pmm

        let base_balance = FixedU256::new_from_fixed_u64(token_a.amount)?;
        let quote_balance = FixedU256::new_from_fixed_u64(token_b.amount)?;
        let pay_quote_amount = FixedU256::new_from_fixed_u64(amount_in)?;

        if quote_balance.inner_u64()? < pay_quote_amount.inner_u64()? {
            return Err(ProgramError::InsufficientFunds);
        }

        let base_reserve = token_swap.base_reserve;
        let quote_reserve = token_swap.quote_reserve;
        let base_target = token_swap.base_target;
        let quote_target = token_swap.quote_target;

        let mut state = PMMState::new(
            token_swap.i,
            token_swap.k,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            token_swap.r,
        );
        adjusted_target(&mut state)?;

        let (receive_base_amount, new_r) = sell_quote_token(state, pay_quote_amount)?;

        let fees = &token_swap.fees;
        let dy_fee;
        match fees.trade_fee(receive_base_amount.inner()) {
            Some(v) => {
                dy_fee = FixedU256::new_from_fixed_u256(v)?;
            }
            None => {
                return Err(ProgramError::InvalidArgument);
            }
        }
        let admin_fee;
        match fees.admin_trade_fee(dy_fee.inner()) {
            Some(v) => {
                admin_fee = FixedU256::new_from_fixed_u256(v)?;
            }
            None => {
                return Err(ProgramError::InvalidArgument);
            }
        }
        let dy_swap_amount = receive_base_amount.checked_sub(dy_fee)?;
        let new_base_target = base_target;
        let new_quote_target = state.q_0;

        if dy_swap_amount.inner_u64()? < minimum_amount_out {
            return Err(SwapError::ExceededSlippage.into());
        }

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            destination_info.clone(),
            swap_destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            amount_in,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            swap_source_info.clone(),
            source_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            dy_swap_amount.inner_u64()?,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            swap_source_info.clone(),
            admin_destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            admin_fee.inner_u64()?,
        )?;

        let new_base_reserve = base_balance.checked_sub(dy_swap_amount)?;
        let new_quote_reserve = quote_balance.checked_add(pay_quote_amount)?;

        let block_timestamp_last: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut base_price_cumulative_last = token_swap.base_price_cumulative_last;
        if token_swap.is_open_twap == TWAP_OPENED {
            let time_elapsed = block_timestamp_last - token_swap.block_timestamp_last;
            if time_elapsed > 0
                && new_base_reserve.inner_u64()? != 0
                && new_quote_reserve.inner_u64()? != 0
            {
                let state = PMMState::new(
                    token_swap.i,
                    token_swap.k,
                    new_base_reserve,
                    new_quote_reserve,
                    new_base_target,
                    new_quote_target,
                    token_swap.r,
                );

                base_price_cumulative_last = base_price_cumulative_last.checked_add(
                    get_mid_price(state)?
                        .checked_mul_floor(FixedU256::new_from_u64(time_elapsed as u64)?)?,
                )?;
            }
        }

        let obj = SwapInfo {
            is_initialized: token_swap.is_initialized,
            is_paused: token_swap.is_paused,
            nonce: token_swap.nonce,
            initial_amp_factor: token_swap.initial_amp_factor,
            target_amp_factor: token_swap.target_amp_factor,
            start_ramp_ts: token_swap.start_ramp_ts,
            stop_ramp_ts: token_swap.stop_ramp_ts,
            future_admin_deadline: token_swap.future_admin_deadline,
            future_admin_key: token_swap.future_admin_key,
            admin_key: token_swap.admin_key,
            token_a: token_swap.token_a,
            token_b: token_swap.token_b,
            pool_mint: token_swap.pool_mint,
            token_a_mint: token_swap.token_a_mint,
            token_b_mint: token_swap.token_b_mint,
            admin_fee_key_a: token_swap.admin_fee_key_a,
            admin_fee_key_b: token_swap.admin_fee_key_b,
            deltafi_token: token_swap.deltafi_token,
            deltafi_mint: token_swap.deltafi_mint,
            fees: token_swap.fees,
            oracle: token_swap.oracle,
            rewards: token_swap.rewards,
            k: token_swap.k,
            i: token_swap.i,
            r: new_r,
            base_target: new_base_target,
            quote_target: new_quote_target,
            base_reserve: new_base_reserve,
            quote_reserve: new_quote_reserve,
            is_open_twap: token_swap.is_open_twap,
            block_timestamp_last,
            base_price_cumulative_last,
        };
        SwapInfo::pack(obj, &mut swap_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes an [Deposit](enum.Instruction.html).
    pub fn process_deposit(
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
        let dest_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let clock_sysvar_info = next_account_info(account_info_iter)?;

        let token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
        if token_swap.is_paused {
            return Err(SwapError::IsPaused.into());
        }
        if *authority_info.key != utils::authority_id(program_id, swap_info.key, token_swap.nonce)?
        {
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

        let _clock = Clock::from_account_info(clock_sysvar_info)?;
        let token_a = utils::unpack_token_account(&token_a_info.data.borrow())?;
        let token_b = utils::unpack_token_account(&token_b_info.data.borrow())?;
        let pool_mint = Self::unpack_mint(&pool_mint_info.data.borrow())?;

        // oracle update

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let (price0_cumulative, price1_cumulative, block_timestamp) =
            token_swap.oracle.current_cumulative_price(
                U256::from(token_a.amount),
                U256::from(token_b.amount),
                current_timestamp,
            );
        let mut swap = token_swap;

        swap.oracle
            .update(price0_cumulative, price1_cumulative, block_timestamp);

        // impl pmm into deposit process
        let mut base_blance = FixedU256::new_from_fixed_u64(token_a.amount)?;
        let mut quote_blance = FixedU256::new_from_fixed_u64(token_b.amount)?;
        let base_in_amount = FixedU256::new_from_fixed_u64(token_a_amount)?;
        let quote_in_amount = FixedU256::new_from_fixed_u64(token_b_amount)?;
        let base_reserve = token_swap.base_reserve;
        let quote_reserve = token_swap.quote_reserve;
        let base_target = token_swap.base_target;
        let quote_target = token_swap.quote_target;

        let (base_adjusted_in_amount, quote_adjusted_in_amount) = get_deposit_adjustment_amount(
            base_in_amount,
            quote_in_amount,
            base_reserve,
            quote_reserve,
            token_swap.i,
        )?;

        base_blance = base_blance.checked_add(base_adjusted_in_amount)?;
        quote_blance = quote_blance.checked_add(quote_adjusted_in_amount)?;

        let (mint_amount, new_base_target, new_quote_target, new_base_reserve, new_quote_reserve) =
            get_buy_shares(
                base_blance,
                quote_blance,
                base_reserve,
                quote_reserve,
                base_target,
                quote_target,
                FixedU256::new_from_fixed_u64(pool_mint.supply)?,
                token_swap.i,
            )?;

        if mint_amount.inner_u64()? < min_mint_amount {
            return Err(SwapError::ExceededSlippage.into());
        }

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_a_info.clone(),
            token_a_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            base_adjusted_in_amount.inner_u64()?,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_b_info.clone(),
            token_b_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            quote_adjusted_in_amount.inner_u64()?,
        )?;
        Self::token_mint_to(
            swap_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            dest_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            mint_amount.inner_u64()?,
        )?;

        let block_timestamp_last: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut base_price_cumulative_last = token_swap.base_price_cumulative_last;
        if token_swap.is_open_twap == TWAP_OPENED {
            let time_elapsed = block_timestamp_last - token_swap.block_timestamp_last;
            if time_elapsed > 0
                && new_base_reserve.inner_u64()? != 0
                && new_quote_reserve.inner_u64()? != 0
            {
                let state = PMMState::new(
                    token_swap.i,
                    token_swap.k,
                    new_base_reserve,
                    new_quote_reserve,
                    new_base_target,
                    new_quote_target,
                    token_swap.r,
                );

                base_price_cumulative_last = base_price_cumulative_last.checked_add(
                    get_mid_price(state)?
                        .checked_mul_floor(FixedU256::new_from_u64(time_elapsed as u64)?)?,
                )?;
            }
        }

        let obj = SwapInfo {
            is_initialized: token_swap.is_initialized,
            is_paused: token_swap.is_paused,
            nonce: token_swap.nonce,
            initial_amp_factor: token_swap.initial_amp_factor,
            target_amp_factor: token_swap.target_amp_factor,
            start_ramp_ts: token_swap.start_ramp_ts,
            stop_ramp_ts: token_swap.stop_ramp_ts,
            future_admin_deadline: token_swap.future_admin_deadline,
            future_admin_key: token_swap.future_admin_key,
            admin_key: token_swap.admin_key,
            token_a: token_swap.token_a,
            token_b: token_swap.token_b,
            pool_mint: token_swap.pool_mint,
            token_a_mint: token_swap.token_a_mint,
            token_b_mint: token_swap.token_b_mint,
            admin_fee_key_a: token_swap.admin_fee_key_a,
            admin_fee_key_b: token_swap.admin_fee_key_b,
            deltafi_token: token_swap.deltafi_token,
            deltafi_mint: token_swap.deltafi_mint,
            fees: token_swap.fees,
            oracle: token_swap.oracle,
            rewards: token_swap.rewards,
            k: token_swap.k,
            i: token_swap.i,
            r: token_swap.r,
            base_target: new_base_target,
            quote_target: new_quote_target,
            base_reserve: new_base_reserve,
            quote_reserve: new_quote_reserve,
            is_open_twap: token_swap.is_open_twap,
            block_timestamp_last,
            base_price_cumulative_last,
        };
        SwapInfo::pack(obj, &mut swap_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes an [Withdraw](enum.Instruction.html).
    pub fn process_withdraw(
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
        let token_program_info = next_account_info(account_info_iter)?;

        let token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
        if *authority_info.key != utils::authority_id(program_id, swap_info.key, token_swap.nonce)?
        {
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
        if *admin_fee_dest_a_info.key != token_swap.admin_fee_key_a {
            return Err(SwapError::InvalidAdmin.into());
        }
        if *admin_fee_dest_b_info.key != token_swap.admin_fee_key_b {
            return Err(SwapError::InvalidAdmin.into());
        }
        let pool_mint = Self::unpack_mint(&pool_mint_info.data.borrow())?;
        if pool_mint.supply == 0 {
            return Err(SwapError::EmptyPool.into());
        }

        let token_a = utils::unpack_token_account(&token_a_info.data.borrow())?;
        let token_b = utils::unpack_token_account(&token_b_info.data.borrow())?;

        let converter = PoolTokenConverter {
            supply: U256::from(pool_mint.supply),
            token_a: U256::from(token_a.amount),
            token_b: U256::from(token_b.amount),
            fees: &token_swap.fees,
        };
        let pool_token_amount_u256 = U256::from(pool_token_amount);
        let (a_amount_u256, a_admin_fee_u256) = converter
            .token_a_rate(pool_token_amount_u256)
            .ok_or(SwapError::CalculationFailure)?;
        let (a_amount, a_admin_fee) = (
            U256::to_u64(a_amount_u256)?,
            U256::to_u64(a_admin_fee_u256)?,
        );
        if a_amount < minimum_token_a_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        let (b_amount_u256, b_admin_fee_u256) = converter
            .token_b_rate(pool_token_amount_u256)
            .ok_or(SwapError::CalculationFailure)?;
        let (b_amount, b_admin_fee) = (
            U256::to_u64(b_amount_u256)?,
            U256::to_u64(b_admin_fee_u256)?,
        );
        if b_amount < minimum_token_b_amount {
            return Err(SwapError::ExceededSlippage.into());
        }

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            token_a_info.clone(),
            dest_token_a_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            a_amount,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            token_a_info.clone(),
            admin_fee_dest_a_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            a_admin_fee,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            token_b_info.clone(),
            dest_token_b_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            b_amount,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            token_b_info.clone(),
            admin_fee_dest_b_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            b_admin_fee,
        )?;
        Self::token_burn(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            pool_mint_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            pool_token_amount,
        )?;
        Ok(())
    }

    /// Processes an [WithdrawOne](enum.Instruction.html).
    pub fn process_withdraw_one(
        program_id: &Pubkey,
        pool_token_amount: u64,
        minimum_token_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let base_token_info = next_account_info(account_info_iter)?;
        let quote_token_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let admin_destination_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let clock_sysvar_info = next_account_info(account_info_iter)?;

        if *base_token_info.key == *quote_token_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        let token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
        if token_swap.is_paused {
            return Err(SwapError::IsPaused.into());
        }
        if *authority_info.key != utils::authority_id(program_id, swap_info.key, token_swap.nonce)?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if *base_token_info.key != token_swap.token_b && *base_token_info.key != token_swap.token_a
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *quote_token_info.key != token_swap.token_b
            && *quote_token_info.key != token_swap.token_a
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *base_token_info.key == token_swap.token_a
            && *admin_destination_info.key != token_swap.admin_fee_key_a
        {
            return Err(SwapError::InvalidAdmin.into());
        }
        if *base_token_info.key == token_swap.token_b
            && *admin_destination_info.key != token_swap.admin_fee_key_b
        {
            return Err(SwapError::InvalidAdmin.into());
        }
        if *pool_mint_info.key != token_swap.pool_mint {
            return Err(SwapError::IncorrectMint.into());
        }
        let pool_mint = Self::unpack_mint(&pool_mint_info.data.borrow())?;
        if pool_token_amount > pool_mint.supply {
            return Err(SwapError::InvalidInput.into());
        }

        let clock = Clock::from_account_info(clock_sysvar_info)?;
        let base_token = utils::unpack_token_account(&base_token_info.data.borrow())?;
        let quote_token = utils::unpack_token_account(&quote_token_info.data.borrow())?;

        let invariant = StableSwap::new(
            token_swap.initial_amp_factor,
            token_swap.target_amp_factor,
            clock.unix_timestamp,
            token_swap.start_ramp_ts,
            token_swap.stop_ramp_ts,
        );
        let (dy, dy_fee) = invariant
            .compute_withdraw_one(
                U256::from(pool_token_amount),
                U256::from(pool_mint.supply),
                U256::from(base_token.amount),
                U256::from(quote_token.amount),
                &token_swap.fees,
            )
            .ok_or(SwapError::CalculationFailure)?;
        let withdraw_fee = token_swap
            .fees
            .withdraw_fee(dy)
            .ok_or(SwapError::CalculationFailure)?;
        let token_amount = U256::to_u64(
            dy.checked_sub(withdraw_fee)
                .ok_or(SwapError::CalculationFailure)?,
        )?;
        if token_amount < minimum_token_amount {
            return Err(SwapError::ExceededSlippage.into());
        }

        let admin_trade_fee = token_swap
            .fees
            .admin_trade_fee(dy_fee)
            .ok_or(SwapError::CalculationFailure)?;
        let admin_withdraw_fee = token_swap
            .fees
            .admin_withdraw_fee(withdraw_fee)
            .ok_or(SwapError::CalculationFailure)?;
        let admin_fee = admin_trade_fee
            .checked_add(admin_withdraw_fee)
            .ok_or(SwapError::CalculationFailure)?;

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            base_token_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            token_amount,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            base_token_info.clone(),
            admin_destination_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            U256::to_u64(admin_fee)?,
        )?;
        Self::token_burn(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            pool_mint_info.clone(),
            authority_info.clone(),
            token_swap.nonce,
            pool_token_amount,
        )?;
        Ok(())
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = AdminInstruction::unpack(input)?;
        match instruction {
            None => Self::process_swap_instruction(program_id, accounts, input),
            Some(admin_instruction) => {
                process_admin_instruction(&admin_instruction, program_id, accounts)
            }
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
                amp_factor,
                fees,
                rewards,
                k,
                i,
                is_open_twap,
            }) => {
                msg!("Instruction: Init");
                Self::process_initialize(
                    program_id,
                    nonce,
                    amp_factor,
                    fees,
                   rewards, k,
                    i,
                    is_open_twap,
                    accounts,
                )
            }
            SwapInstruction::Swap(SwapData {
                amount_in,
                minimum_amount_out,
                swap_direction,
            }) => {
                msg!("Instruction: Swap");
                Self::process_swap(
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
                Self::process_deposit(
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
                Self::process_withdraw(
                    program_id,
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                    accounts,
                )
            }
            SwapInstruction::WithdrawOne(WithdrawOneData {
                pool_token_amount,
                minimum_token_amount,
            }) => {
                msg!("Instruction: Withdraw One");
                Self::process_withdraw_one(
                    program_id,
                    pool_token_amount,
                    minimum_token_amount,
                    accounts,
                )
            }
        }
    }
}

impl PrintProgramError for SwapError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            SwapError::AlreadyInUse => msg!("Error: Swap account already in use"),
            SwapError::InvalidAdmin => {
                msg!("Error: Address of the admin fee account is incorrect")
            }
            SwapError::InvalidOwner => {
                msg!("Error: The input account owner is not the program address")
            }
            SwapError::InvalidOutputOwner => {
                msg!("Error: Output pool account owner cannot be the program address")
            }
            SwapError::InvalidProgramAddress => {
                msg!("Error: Invalid program address generated from nonce and key")
            }
            SwapError::ExpectedMint => {
                msg!("Error: Deserialized account is not an SPL Token mint")
            }
            SwapError::ExpectedAccount => {
                msg!("Error: Deserialized account is not an SPL Token account")
            }
            SwapError::EmptySupply => msg!("Error: Input token account empty"),
            SwapError::EmptyPool => msg!("Error: Pool token supply is 0"),
            SwapError::InvalidSupply => msg!("Error: Pool token mint has a non-zero supply"),
            SwapError::RepeatedMint => msg!("Error: Swap input token accounts have the same mint"),
            SwapError::InvalidDelegate => msg!("Error: Token account has a delegate"),
            SwapError::InvalidInput => msg!("Error: InvalidInput"),
            SwapError::IncorrectSwapAccount => {
                msg!("Error: Address of the provided swap token account is incorrect")
            }
            SwapError::IncorrectRewardAccount => {
                msg!("Error: Address of the reward token account is incorrect")
            }
            SwapError::IncorrectMint => {
                msg!("Error: Address of the provided token mint is incorrect")
            }
            SwapError::CalculationFailure => msg!("Error: CalculationFailure"),
            SwapError::InvalidInstruction => msg!("Error: InvalidInstruction"),
            SwapError::ExceededSlippage => {
                msg!("Error: Swap instruction exceeds desired slippage limit")
            }
            SwapError::InvalidCloseAuthority => msg!("Error: Token account has a close authority"),
            SwapError::InvalidFreezeAuthority => {
                msg!("Error: Pool token mint has a freeze authority")
            }
            SwapError::ConversionFailure => msg!("Error: Conversion to or from u64 failed"),
            SwapError::Unauthorized => {
                msg!("Error: Account is not authorized to execute this instruction")
            }
            SwapError::IsPaused => msg!("Error: Swap pool is paused"),
            SwapError::RampLocked => msg!("Error: Ramp is locked in this time period"),
            SwapError::InsufficientRampTime => msg!("Error: Insufficient ramp time"),
            SwapError::ActiveTransfer => msg!("Error: Active admin transfer in progress"),
            SwapError::NoActiveTransfer => msg!("Error: No active admin transfer in progress"),
            SwapError::AdminDeadlineExceeded => msg!("Error: Admin transfer deadline exceeded"),
            SwapError::MismatchedDecimals => msg!("Error: Token mints must have same decimals"),

            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use solana_sdk::account::Account;
    use spl_token::{
        error::TokenError,
        instruction::{approve, mint_to, revoke, set_authority, AuthorityType},
    };

    use super::*;
    use crate::{
        instruction::{deposit, swap, withdraw, withdraw_one},
        utils::{
            test_utils::*, DEFAULT_BASE_POINT, DEFAULT_TOKEN_DECIMALS, SWAP_DIRECTION_SELL_BASE,
            SWAP_DIRECTION_SELL_QUOTE,
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
        let token_program = (TOKEN_PROGRAM_ID, Account::default());
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
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            amp_factor,
            token_a_amount,
            token_b_amount,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
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

        // invalid amp factors
        {
            let old_initial_amp_factor = accounts.initial_amp_factor;
            accounts.initial_amp_factor = MIN_AMP - 1;
            // amp factor too low
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.initialize_swap()
            );
            accounts.initial_amp_factor = MAX_AMP + 1;
            // amp factor too high
            assert_eq!(
                Err(SwapError::InvalidInput.into()),
                accounts.initialize_swap()
            );
            accounts.initial_amp_factor = old_initial_amp_factor;
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

        // uninitialized deltafi mint
        {
            let old_account = accounts.deltafi_mint_account;
            accounts.deltafi_mint_account = Account::default();
            assert_eq!(
                Err(SwapError::ExpectedMint.into()),
                accounts.initialize_swap(),
            );
            accounts.deltafi_mint_account = old_account;
        }

        // token A account owner is not swap authority
        {
            let (_token_a_key, token_a_account) = mint_token(
                &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
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
                create_mint(&TOKEN_PROGRAM_ID, &user_key, DEFAULT_TOKEN_DECIMALS, None);
            let old_mint = accounts.pool_mint_account;
            accounts.pool_mint_account = pool_mint_account;
            assert_eq!(
                Err(SwapError::InvalidOwner.into()),
                accounts.initialize_swap()
            );
            accounts.pool_mint_account = old_mint;
        }

        // deltafi mint authority is not swap authority
        {
            let (_deltafi_mint_key, deltafi_mint_account) =
                create_mint(&TOKEN_PROGRAM_ID, &user_key, DEFAULT_TOKEN_DECIMALS, None);
            let old_account = accounts.deltafi_mint_account;
            accounts.deltafi_mint_account = deltafi_mint_account;
            assert_eq!(
                Err(SwapError::InvalidOwner.into()),
                accounts.initialize_swap()
            );
            accounts.deltafi_mint_account = old_account;
        }

        // pool mint token has freeze authority
        {
            let (_pool_mint_key, pool_mint_account) = create_mint(
                &TOKEN_PROGRAM_ID,
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

        // deltafi mint token has freeze authority
        {
            let (_deltafi_mint_key, deltafi_mint_account) = create_mint(
                &TOKEN_PROGRAM_ID,
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                Some(&user_key),
            );
            let old_account = accounts.deltafi_mint_account;
            accounts.deltafi_mint_account = deltafi_mint_account;
            assert_eq!(
                Err(SwapError::InvalidFreezeAuthority.into()),
                accounts.initialize_swap()
            );
            accounts.deltafi_mint_account = old_account;
        }

        // empty token A account
        {
            let (_token_a_key, token_a_account) = mint_token(
                &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
                &accounts.authority_key,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            accounts.pool_mint_account = pool_mint_account;

            let (_empty_pool_token_key, empty_pool_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &accounts.pool_mint_key,
                &mut accounts.pool_mint_account,
                &accounts.authority_key,
                &user_key,
                0,
            );

            let (_pool_token_key, pool_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                    &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
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

        // mismatched mint decimals
        {
            let (bad_mint_key, mut bad_mint_account) =
                create_mint(&TOKEN_PROGRAM_ID, &accounts.authority_key, 2, None);
            let (bad_deltafi_mint_key, mut bad_deltafi_mint_account) =
                create_mint(&TOKEN_PROGRAM_ID, &accounts.authority_key, 2, None);

            // Pool mint decimal does not match
            let old_pool_mint_key = accounts.pool_mint_key;
            let old_pool_mint_account = accounts.pool_mint_account;
            accounts.pool_mint_key = bad_mint_key;
            accounts.pool_mint_account = bad_mint_account.clone();

            assert_eq!(
                Err(SwapError::MismatchedDecimals.into()),
                accounts.initialize_swap()
            );

            accounts.pool_mint_key = old_pool_mint_key;
            accounts.pool_mint_account = old_pool_mint_account;

            let old_deltafi_mint_key = accounts.deltafi_mint_key;
            let old_deltafi_mint_account = accounts.deltafi_mint_account;
            accounts.deltafi_mint_key = bad_deltafi_mint_key;
            accounts.deltafi_mint_account = bad_deltafi_mint_account.clone();

            assert_eq!(
                Err(SwapError::MismatchedDecimals.into()),
                accounts.initialize_swap()
            );

            accounts.deltafi_mint_key = old_deltafi_mint_key;
            accounts.deltafi_mint_account = old_deltafi_mint_account;

            // Token a mint decimal does not match token b decimals
            let (bad_token_key, bad_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &bad_mint_key,
                &mut bad_mint_account,
                &accounts.authority_key,
                &accounts.authority_key,
                10,
            );

            let old_token_a_key = accounts.token_a_key;
            let old_token_a_account = accounts.token_a_account;
            let old_token_a_mint_key = accounts.token_a_mint_key;
            let old_token_a_mint_account = accounts.token_a_mint_account;
            accounts.token_a_key = bad_token_key;
            accounts.token_a_account = bad_token_account;
            accounts.token_a_mint_key = bad_mint_key;
            accounts.token_a_mint_account = bad_mint_account;

            assert_eq!(
                Err(SwapError::MismatchedDecimals.into()),
                accounts.initialize_swap()
            );

            accounts.token_a_key = old_token_a_key;
            accounts.token_a_account = old_token_a_account;
            accounts.token_a_mint_key = old_token_a_mint_key;
            accounts.token_a_mint_account = old_token_a_mint_account;

            // Deltafi token does not match with token a decimals
            let (bad_deltafi_token_key, bad_deltafi_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
                &bad_deltafi_mint_key,
                &mut bad_deltafi_mint_account,
                &accounts.authority_key,
                &user_key,
                10,
            );

            let old_deltafi_token_key = accounts.deltafi_token_key;
            let old_deltafi_token_account = accounts.deltafi_token_account;
            let old_deltafi_mint_key = accounts.deltafi_mint_key;
            let old_deltafi_mint_account = accounts.deltafi_mint_account;
            accounts.deltafi_token_key = bad_deltafi_token_key;
            accounts.deltafi_token_account = bad_deltafi_token_account;
            accounts.deltafi_mint_key = bad_deltafi_mint_key;
            accounts.deltafi_mint_account = bad_deltafi_mint_account;

            assert_eq!(
                Err(SwapError::MismatchedDecimals.into()),
                accounts.initialize_swap()
            );

            accounts.deltafi_token_key = old_deltafi_token_key;
            accounts.deltafi_token_account = old_deltafi_token_account;
            accounts.deltafi_mint_key = old_deltafi_mint_key;
            accounts.deltafi_mint_account = old_deltafi_mint_account;
        }

        // create swap with same token A and B
        {
            let (_token_a_repeat_key, token_a_repeat_account) = mint_token(
                &TOKEN_PROGRAM_ID,
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
        assert_eq!(swap_info.future_admin_deadline, ZERO_TS);
        assert_eq!(swap_info.future_admin_key, Pubkey::default());
        assert_eq!(swap_info.admin_key, accounts.admin_key);
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
        assert_eq!(swap_info.k, default_k());
        assert_eq!(swap_info.i, default_i());
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
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            amp_factor,
            token_a_amount,
            token_b_amount,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
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
                &TOKEN_PROGRAM_ID,
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
                        &TOKEN_PROGRAM_ID,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &token_b_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &accounts.pool_mint_key,
                        &pool_key,
                        deposit_a,
                        deposit_b,
                        min_mint_amount,
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
                Err(ProgramError::InvalidAccountData),
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
                        deposit_a,
                        deposit_b,
                        min_mint_amount,
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
                &TOKEN_PROGRAM_ID,
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
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            amp_factor,
            token_a_amount,
            token_b_amount,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
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
                &TOKEN_PROGRAM_ID,
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
                        &TOKEN_PROGRAM_ID,
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
                Err(ProgramError::InvalidAccountData),
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
                &TOKEN_PROGRAM_ID,
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
    fn test_sell_buy() {
        let user_key = pubkey_rand();
        let swapper_key = pubkey_rand();
        let amp_factor = 85;
        let token_a_amount = FixedU256::new_from_u64(1000).unwrap();
        let token_b_amount = FixedU256::new_from_u64(1000).unwrap();
        let k = FixedU256::one()
            .checked_mul_floor(FixedU256::new(1.into()))
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();
        let i = FixedU256::one();

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

        let mut accounts = SwapAccountInfo::new(
            &user_key,
            amp_factor,
            token_a_amount.inner_u64().unwrap(),
            token_b_amount.inner_u64().unwrap(),
            swap_fees,
            DEFAULT_TEST_REWARDS,
            k,
            i,
        );

        let initial_a = token_a_amount
            .checked_div_ceil(FixedU256::new(2.into()))
            .unwrap();
        let initial_b = token_b_amount
            .checked_div_ceil(FixedU256::new(2.into()))
            .unwrap();
        let mut swap_direction = SWAP_DIRECTION_SELL_BASE;
        let pay_amount = FixedU256::new_from_u64(100).unwrap();
        let minimum_b_amount = pay_amount
            .checked_div_ceil(FixedU256::new(2.into()))
            .unwrap();

        let swap_token_a_key = accounts.token_a_key;
        let swap_token_b_key = accounts.token_b_key;

        accounts.initialize_swap().unwrap();
        let initial_info = SwapInfo::unpack(&accounts.swap_account.data).unwrap();

        let swap_token_a = utils::unpack_token_account(&accounts.token_a_account.data).unwrap();
        let token_a_amount = swap_token_a.amount;

        let swap_token_b = utils::unpack_token_account(&accounts.token_b_account.data).unwrap();
        let token_b_amount = swap_token_b.amount;

        // let swap_token_admin = utils::unpack_token_account(&accounts.admin_account.data).unwrap();
        // let token_admin_amount = swap_token_admin.amount;

        let swap_token_admin_fee_a =
            utils::unpack_token_account(&accounts.admin_fee_a_account.data).unwrap();
        let token_admin_fee_a_amount = swap_token_admin_fee_a.amount;

        let swap_token_admin_fee_b =
            utils::unpack_token_account(&accounts.admin_fee_b_account.data).unwrap();
        let token_admin_fee_b_amount = swap_token_admin_fee_b.amount;

        assert_eq!(token_a_amount, 1000000000);
        assert_eq!(token_b_amount, 1000000000);
        // assert_eq!(token_admin_amount, 1000);
        assert_eq!(token_admin_fee_a_amount, 0);
        assert_eq!(token_admin_fee_b_amount, 0);
        assert_eq!(initial_info.base_target.inner_u64().unwrap(), 1000000000);
        assert_eq!(initial_info.quote_target.inner_u64().unwrap(), 1000000000);
        assert_eq!(initial_info.base_reserve.inner_u64().unwrap(), 1000000000);
        assert_eq!(initial_info.quote_reserve.inner_u64().unwrap(), 1000000000);

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
            initial_a.inner_u64().unwrap(),
            initial_b.inner_u64().unwrap(),
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
                pay_amount.inner_u64().unwrap(),
                minimum_b_amount.inner_u64().unwrap(),
                swap_direction,
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
            589
        );
        assert_eq!(
            token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            1100
        );
        assert_eq!(token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(), 910);
        assert_eq!(token_admin_fee_a_amount, 0);
        assert_eq!(token_admin_fee_b_amount, 0);
        assert_eq!(swap_info.base_target.into_u64_ceil().unwrap(), 1000);
        assert_eq!(swap_info.quote_target.into_u64_ceil().unwrap(), 1000);
        assert_eq!(swap_info.base_reserve.into_u64_ceil().unwrap(), 1100);
        assert_eq!(swap_info.quote_reserve.into_u64_ceil().unwrap(), 911);

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
                pay_amount.inner_u64().unwrap(),
                minimum_b_amount.inner_u64().unwrap(),
                swap_direction,
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

        let user_token_a = utils::unpack_token_account(&token_a_account.data).unwrap();
        let user_token_a_amount = user_token_a.amount;

        let user_token_b = utils::unpack_token_account(&token_b_account.data).unwrap();
        let user_token_b_amount = user_token_b.amount;

        assert_eq!(
            user_token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            503
        );
        assert_eq!(
            user_token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            489
        );
        assert_eq!(token_a_amount.checked_div(DEFAULT_BASE_POINT).unwrap(), 996);
        assert_eq!(
            token_b_amount.checked_div(DEFAULT_BASE_POINT).unwrap(),
            1010
        );
        assert_eq!(token_admin_fee_a_amount, 52);
        assert_eq!(token_admin_fee_b_amount, 0);
        assert_eq!(swap_info.base_target.into_u64_ceil().unwrap(), 1000);
        assert_eq!(swap_info.quote_target.into_u64_ceil().unwrap(), 1006);
        assert_eq!(swap_info.base_reserve.into_u64_ceil().unwrap(), 997);
        assert_eq!(swap_info.quote_reserve.into_u64_ceil().unwrap(), 1011);
    }

    #[test]
    fn test_swap() {
        let user_key = pubkey_rand();
        let swapper_key = pubkey_rand();
        let amp_factor = 85;
        let token_a_amount = 100;
        let token_b_amount = 10000;
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            amp_factor,
            token_a_amount,
            token_b_amount,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
        );
        let initial_a = token_a_amount;
        let initial_b = token_b_amount;
        let minimum_b_amount = initial_b / 20;
        let swap_direction = SWAP_DIRECTION_SELL_BASE;

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
                &TOKEN_PROGRAM_ID,
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
                Err(ProgramError::InvalidAccountData),
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
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
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
                        &TOKEN_PROGRAM_ID,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &token_a_key,
                        &token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &accounts.admin_fee_b_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
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
                        &TOKEN_PROGRAM_ID,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &wrong_admin_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
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
                        &TOKEN_PROGRAM_ID,
                        &accounts.swap_key,
                        &accounts.authority_key,
                        &token_a_key,
                        &accounts.token_a_key,
                        &accounts.token_b_key,
                        &token_b_key,
                        &accounts.deltafi_token_key,
                        &accounts.deltafi_mint_key,
                        &accounts.admin_fee_b_key,
                        initial_a,
                        minimum_b_amount,
                        swap_direction,
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
        let mut accounts = SwapAccountInfo::new(
            &user_key,
            amp_factor,
            token_a_amount,
            token_b_amount,
            DEFAULT_TEST_FEES,
            DEFAULT_TEST_REWARDS,
            default_k(),
            default_i(),
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
                &TOKEN_PROGRAM_ID,
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
                &TOKEN_PROGRAM_ID,
                &foreign_authority,
                DEFAULT_TOKEN_DECIMALS,
                None,
            );
            let (foreign_token_key, foreign_token_account) = mint_token(
                &TOKEN_PROGRAM_ID,
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
                        &TOKEN_PROGRAM_ID,
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
                Err(ProgramError::InvalidAccountData),
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
                &TOKEN_PROGRAM_ID,
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
                .withdraw_fee(withdraw_one_amount_before_fees)
                .unwrap();
            let expected_withdraw_one_amount =
                withdraw_one_amount_before_fees - withdraw_one_withdraw_fee;
            let expected_admin_fee = U256::to_u64(
                DEFAULT_TEST_FEES
                    .admin_trade_fee(withdraw_one_trade_fee)
                    .unwrap()
                    + DEFAULT_TEST_FEES
                        .admin_withdraw_fee(withdraw_one_withdraw_fee)
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
