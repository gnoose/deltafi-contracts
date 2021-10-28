//! Calculation functions

use std::cmp::Ordering;

use crate::{
    error::SwapError,
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
};
use solana_program::program_error::ProgramError;

/// Get target amount given quote amount.
///
/// target_amount = market_price * quote_amount * (1 - slope
///         + slope * (target_reserve^2 / future_reserve / current_reserve))
/// where quote_amount = future_reserve - current_reserve.
///
/// # Arguments
///
/// * target_reserve - initial reserve position to track divergent loss.
/// * future_reserve - reserve position after the current quoted trade.
/// * current_reserve - current reserve position.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// target amount determined by the pricing function.
pub fn get_target_amount(
    target_reserve: Decimal,
    future_reserve: Decimal,
    current_reserve: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    // TODO: add code to enforce target_reserve >= future_reserve >= current_reserve > 0
    let fair_amount = future_reserve
        .try_sub(current_reserve)?
        .try_mul(market_price)?;
    if slope.is_zero() {
        return Ok(fair_amount);
    }
    // TODO: current_reserve should be try_ceil_div. Need to add this function to Decimal and update here.
    let penalty_ratio = target_reserve
        .try_mul(target_reserve)?
        .try_div(future_reserve)?
        .try_div(current_reserve)?;
    let penalty = penalty_ratio.try_mul(slope)?;
    fair_amount.try_mul(penalty.try_add(Decimal::one())?.try_sub(slope)?)
}

/// Get target amount given quote amount in reserve direction.
///
/// # Arguments
///
/// * target_reserve - initial reserve position to track divergent loss.
/// * current_reserve - current reserve position.
/// * quote_amount - quote amount.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// target amount determined by the pricing function.
pub fn get_target_amount_reverse_direction(
    target_reserve: Decimal,
    current_reserve: Decimal,
    quote_amount: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if target_reserve.is_zero() {
        return Err(SwapError::CalculationFailure.into());
    }
    if quote_amount.is_zero() {
        return Ok(Decimal::zero());
    }

    let fair_amount = quote_amount.try_mul(market_price)?;
    if slope.is_zero() {
        return Ok(fair_amount.min(current_reserve));
    }

    if slope == Decimal::one() {
        let adjusted_ratio = fair_amount
            .try_mul(current_reserve)?
            .try_div(target_reserve)?
            .try_div(target_reserve)?;
        return current_reserve
            .try_mul(adjusted_ratio)?
            .try_div(adjusted_ratio.try_add(Decimal::one())?);
    }

    let future_reserve = target_reserve
        .try_mul(target_reserve)?
        .try_div(current_reserve)?
        .try_mul(slope)?
        .try_add(fair_amount)?;
    let mut adjusted_reserve = Decimal::one().try_sub(slope)?.try_mul(current_reserve)?;

    let is_smaller = if adjusted_reserve < future_reserve {
        adjusted_reserve = future_reserve.try_sub(adjusted_reserve)?;
        true
    } else {
        adjusted_reserve = adjusted_reserve.try_sub(future_reserve)?;
        false
    };

    let square_root = Decimal::one()
        .try_sub(slope)?
        .try_mul(4)?
        .try_mul(slope)?
        .try_mul(target_reserve)?
        .try_mul(target_reserve)?;
    let square_root = adjusted_reserve
        .try_mul(adjusted_reserve)?
        .try_add(square_root)?
        .sqrt()?;

    let denominator = Decimal::one().try_sub(slope)?.try_mul(2)?;
    let numerator = if is_smaller {
        square_root.try_sub(adjusted_reserve)?
    } else {
        adjusted_reserve.try_add(square_root)?
    };

    let target_reserve = numerator.try_div(denominator)?;

    match target_reserve.cmp(&current_reserve) {
        Ordering::Greater => Ok(Decimal::zero()),
        _ => Ok(current_reserve.try_sub(target_reserve)?),
    }
}

/// Get adjusted target reserve given quote amount.
///
/// # Arguments
///
/// * current_reserve - current reserve position.
/// * quote_amount - quote amount.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// adjusted target reserve.
pub fn get_target_reserve(
    current_reserve: Decimal,
    quote_amount: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if current_reserve.is_zero() {
        return Ok(Decimal::zero());
    }
    if slope.is_zero() {
        return quote_amount.try_mul(market_price)?.try_add(current_reserve);
    }

    let square_root = quote_amount
        .try_mul(market_price)?
        .try_mul(slope)?
        .try_mul(4)?
        .try_div(current_reserve)?
        .try_add(Decimal::one())?
        .sqrt()?;

    let premium = square_root
        .try_sub(Decimal::one())?
        .try_div(2)?
        .try_div(slope)?
        .try_add(Decimal::one())?;

    premium.try_mul(current_reserve)
}
