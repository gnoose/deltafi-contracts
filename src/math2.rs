//! math formulas for proactive market maker
use num_traits::Zero;
use solana_program::program_error::ProgramError;

use crate::bn::FixedU64;

/// div with ceil
pub fn checked_ceil_div(owner: u64, other: u64) -> Result<u64, ProgramError> {
    if other.is_zero() {
        return Err(ProgramError::InvalidArgument);
    }
    let rem;
    match owner.checked_rem(other) {
        Some(v) => {
            rem = v;
        }
        None => {
            return Err(ProgramError::InvalidArgument);
        }
    }
    let quotient = checked_floor_div(owner, other)?;
    if rem == 0 {
        Ok(quotient)
    } else {
        Ok(checked_bn_add(quotient, 1).unwrap())
    }
}

/// div with floor
pub fn checked_floor_div(owner: u64, other: u64) -> Result<u64, ProgramError> {
    if other.is_zero() {
        return Err(ProgramError::InvalidArgument);
    }
    match owner.checked_div(other) {
        Some(v) => Ok(v),
        None => Err(ProgramError::InvalidArgument),
    }
}

/// mul with ProgramError
pub fn checked_bn_mul(owner: u64, other: u64) -> Result<u64, ProgramError> {
    match owner.checked_mul(other) {
        Some(v) => Ok(v),
        None => Err(ProgramError::InvalidArgument),
    }
}

/// add with ProgramError
pub fn checked_bn_add(owner: u64, other: u64) -> Result<u64, ProgramError> {
    match owner.checked_add(other) {
        Some(v) => Ok(v),
        None => Err(ProgramError::InvalidArgument),
    }
}

/// sub with ProgramError
pub fn checked_bn_sub(owner: u64, other: u64) -> Result<u64, ProgramError> {
    match owner.checked_sub(other) {
        Some(v) => Ok(v),
        None => Err(ProgramError::InvalidArgument),
    }
}

/// calculate sqrt
pub fn sqrt(owner: u64) -> Result<u64, ProgramError> {
    let mut z = checked_floor_div(checked_bn_add(owner, 1)?, 2)?;

    let mut y = owner;

    while z < y {
        y = z;
        z = checked_floor_div(checked_bn_add(checked_floor_div(owner, z)?, z)?, 2)?;
    }

    Ok(y)
}

/// calculate deposit amount according to the reserve amount
//      a_reserve = 0 & b_reserve = 0 => (a_amount, b_amount)
//      a_reserve > 0 & b_reserve = 0 => (a_amount, 0)
//      a_reserve > 0 & b_reserve > 0 => (a_amount*ratio1, b_amount*ratio2)
pub fn get_deposit_adjustment_amount(
    base_in_amount: FixedU64,
    quote_in_amount: FixedU64,
    base_reserve_amount: FixedU64,
    quote_reserve_amount: FixedU64,
    i: FixedU64,
) -> Result<(FixedU64, FixedU64), ProgramError> {
    if quote_reserve_amount.into_real_u64_ceil().is_zero()
        && base_reserve_amount.into_real_u64_ceil().is_zero()
    {
        let shares;
        if quote_in_amount.into_real_u64_ceil()
            < base_in_amount.checked_mul_floor(i)?.into_real_u64_ceil()
        {
            shares = quote_in_amount.checked_div_floor(i)?;
        } else {
            shares = base_in_amount;
        }
        let base_adjusted_in_amount = shares;
        let quote_adjusted_in_amount = shares.checked_mul_floor(i)?;

        return Ok((base_adjusted_in_amount, quote_adjusted_in_amount));
    }

    if quote_reserve_amount.into_real_u64_ceil() > 0 && base_reserve_amount.into_real_u64_ceil() > 0
    {
        let base_increase_ratio = base_in_amount.checked_div_floor(base_reserve_amount)?;
        let quote_increase_ratio = quote_in_amount.checked_div_floor(quote_reserve_amount)?;

        let new_quote_increase_ratio =
            quote_increase_ratio.take_and_scale(base_increase_ratio.precision())?;
        if base_increase_ratio.inner() <= new_quote_increase_ratio.inner() {
            Ok((
                base_in_amount,
                quote_reserve_amount.checked_mul_floor(base_increase_ratio)?,
            ))
        } else {
            Ok((
                base_reserve_amount.checked_mul_floor(quote_increase_ratio)?,
                quote_in_amount,
            ))
        }
    } else {
        Ok((base_in_amount, quote_in_amount))
    }
}

/// buy shares [round down] - mint amount for lp - sp
#[allow(clippy::too_many_arguments)]
pub fn get_buy_shares(
    base_balance: FixedU64,
    quote_balance: FixedU64,
    base_reserve: FixedU64,
    quote_reserve: FixedU64,
    base_target: FixedU64,
    quote_target: FixedU64,
    total_supply: FixedU64,
    i: FixedU64,
) -> Result<(FixedU64, FixedU64, FixedU64, FixedU64, FixedU64), ProgramError> {
    let base_input = base_balance.checked_sub(base_reserve)?;
    let quote_input = quote_balance.checked_sub(quote_reserve)?;

    let mut share = FixedU64::zero();
    let mut new_base_target = base_target;
    let mut new_quote_target = quote_target;
    if total_supply.into_real_u64_ceil().is_zero() {
        // case 1. initial supply
        if quote_balance.into_real_u64_ceil()
            < base_balance.checked_mul_floor(i)?.into_real_u64_ceil()
        {
            share = quote_balance.checked_div_floor(i)?;
        } else {
            share = base_balance;
        }
        new_base_target = share;
        new_quote_target = share.checked_mul_floor(i)?;
    } else if base_reserve.into_real_u64_ceil() > 0 && quote_reserve.into_real_u64_ceil() > 0 {
        let base_input_ratio = base_input.checked_div_floor(base_reserve)?;
        let quote_input_ratio = quote_input.checked_div_floor(quote_reserve)?;
        let mint_ratio;
        let new_quote_input_ratio =
            quote_input_ratio.take_and_scale(base_input_ratio.precision())?;
        if new_quote_input_ratio.inner() < base_input_ratio.inner() {
            mint_ratio = quote_input_ratio;
        } else {
            mint_ratio = base_input_ratio;
        }
        share = total_supply.checked_mul_floor(mint_ratio)?;
        new_base_target =
            new_base_target.checked_add(new_base_target.checked_mul_floor(mint_ratio)?)?;
        new_quote_target =
            new_quote_target.checked_add(new_quote_target.checked_mul_floor(mint_ratio)?)?;
    }

    let new_base_reserve = base_balance;
    let new_quote_reserve = quote_balance;
    Ok((
        share,
        new_base_target,
        new_quote_target,
        new_base_reserve,
        new_quote_reserve,
    ))
}

/// Integrate dodo curve from V1 to V2
pub fn general_integrate(
    v0: FixedU64,
    v1: FixedU64,
    v2: FixedU64,
    i: FixedU64,
    k: FixedU64,
) -> Result<FixedU64, ProgramError> {
    let fair_amount = i.checked_mul_floor(v1.checked_sub(v2)?)?;

    let v0v0v1v2 = v0
        .checked_mul_floor(v0)?
        .checked_div_floor(v1)?
        .checked_div_ceil(v2)?;

    let penalty = k.checked_mul_floor(v0v0v1v2)?; // k(V0^2/V1/V2)

    fair_amount.checked_mul_floor(FixedU64::one().checked_sub(k)?.checked_add(penalty)?)
}

/// Follow the integration function above
pub fn solve_quadratic_function_for_target(
    v1: FixedU64,
    delta: FixedU64,
    i: FixedU64,
    k: FixedU64,
) -> Result<FixedU64, ProgramError> {
    if v1.into_real_u64_ceil().is_zero() {
        return Ok(FixedU64::zero());
    }

    if k.into_real_u64_ceil().is_zero() {
        return v1.checked_add(i.checked_mul_floor(delta)?);
    }

    let sqrt;
    let ki = k
        .checked_mul_floor(FixedU64::new(4))?
        .checked_mul_floor(i)?;

    if ki.into_real_u64_ceil().is_zero() {
        sqrt = FixedU64::one();
    } else if ki.checked_mul_floor(delta)?.checked_div_floor(ki)? == delta {
        sqrt = ki
            .checked_mul_floor(delta)?
            .checked_div_floor(v1)?
            .checked_add(FixedU64::one())?
            .sqrt()?;
    } else {
        sqrt = ki
            .checked_div_floor(v1)?
            .checked_mul_floor(delta)?
            .checked_add(FixedU64::one())?
            .sqrt()?;
    }

    let premium = sqrt
        .checked_sub(FixedU64::one())?
        .checked_div_floor(k.checked_mul_floor(FixedU64::new(2))?)?
        .checked_add(FixedU64::one())?;

    v1.checked_mul_floor(premium)
}

/// Follow the integration expression above
pub fn solve_quadratic_function_for_trade(
    v0: FixedU64,
    v1: FixedU64,
    delta: FixedU64,
    i: FixedU64,
    k: FixedU64,
) -> Result<FixedU64, ProgramError> {
    if v0.into_real_u64_ceil().is_zero() {
        return Ok(FixedU64::zero());
    }

    if delta.into_real_u64_ceil().is_zero() {
        return Ok(FixedU64::zero());
    }

    if k.into_real_u64_ceil().is_zero() {
        if i.checked_mul_floor(delta)?.into_real_u64_ceil() > v1.into_real_u64_ceil() {
            return Ok(v1);
        } else {
            return i.checked_mul_floor(delta);
        }
    }

    if k.into_real_u64_ceil() == 1 {
        let temp;
        let i_delta = i.checked_mul_floor(delta)?;
        if i_delta.into_real_u64_ceil().is_zero() {
            temp = FixedU64::zero();
        } else if i_delta
            .checked_mul_floor(v1)?
            .checked_div_floor(i_delta)?
            .into_real_u64_ceil()
            == v1.into_real_u64_ceil()
        {
            temp = i_delta
                .checked_mul_floor(v1)?
                .checked_div_floor(v0.checked_mul_floor(v0)?)?;
        } else {
            temp = delta
                .checked_mul_floor(v1)?
                .checked_div_floor(v0)?
                .checked_mul_floor(i)?
                .checked_div_floor(v0)?;
        }
        return v1
            .checked_mul_floor(temp)?
            .checked_div_floor(temp.checked_add(FixedU64::one())?);
    }

    let part_2 = k
        .checked_mul_floor(v0)?
        .checked_div_floor(v1)?
        .checked_mul_floor(v0)?
        .checked_add(i.checked_mul_floor(delta)?)?; // kQ0^2/Q1-i*deltaB

    let mut b_abs = FixedU64::one().checked_sub(k)?.checked_mul_floor(v1)?; // (1-k)Q1

    let b_sig;
    if b_abs >= part_2 {
        b_abs = b_abs.checked_sub(part_2)?;
        b_sig = false;
    } else {
        b_abs = part_2.checked_sub(b_abs)?;
        b_sig = true;
    }

    b_abs = b_abs.checked_div_floor(FixedU64::one())?;

    let mut square_root = FixedU64::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU64::new(4))?
        .checked_mul_floor(k.checked_mul_floor(v0)?.checked_mul_floor(v0)?)?; // 4(1-k)kQ0^2

    square_root = b_abs
        .checked_mul_floor(b_abs)?
        .checked_add(square_root)?
        .sqrt()?;

    let denominator = FixedU64::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU64::new(2))?; // 2(1-k)
    let numerator;

    if b_sig {
        numerator = square_root.checked_sub(b_abs)?;
    } else {
        numerator = b_abs.checked_add(square_root)?;
    }

    let v2 = numerator.checked_div_ceil(denominator)?;
    if v2.into_real_u64_ceil() > v1.into_real_u64_ceil() {
        Ok(FixedU64::zero())
    } else {
        Ok(v1.checked_sub(v2)?)
    }
}

#[cfg(feature = "test-bpf")]
mod tests {
    use super::*;
    use crate::utils::{
        test_utils::{default_i, default_k},
        DEFAULT_TOKEN_DECIMALS,
    };

    #[test]
    fn basic() {
        let q0: FixedU64 = FixedU64::new_from_int(5000, DEFAULT_TOKEN_DECIMALS).unwrap();
        let q1: FixedU64 = FixedU64::new_from_int(5000, DEFAULT_TOKEN_DECIMALS).unwrap();
        let i: FixedU64 = default_i();
        let delta_b: FixedU64 = FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap();
        let k: FixedU64 = default_k();

        assert_eq!(
            get_deposit_adjustment_amount(
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(0, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(0, DEFAULT_TOKEN_DECIMALS).unwrap(),
                i
            )
            .unwrap(),
            (
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap()
            )
        );

        assert_eq!(
            get_buy_shares(
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(0, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(0, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::zero(),
                FixedU64::zero(),
                FixedU64::zero(),
                default_i()
            )
            .unwrap(),
            (
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
            )
        );

        //  above result is for initialize result
        //  Input
        //  token_a_amount = 100, token_b_amount = 10000, k = 0.5, i = 100,
        //  Result
        //  base_target = 100, base_reserve = 100, quote_target = 10000, quote_reserve = 10000
        //  pool_mint_supply = 100

        assert_eq!(
            get_deposit_adjustment_amount(
                FixedU64::new_from_int(10, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(1000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                i
            )
            .unwrap(),
            (
                FixedU64::new_from_int(10, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(1000, DEFAULT_TOKEN_DECIMALS).unwrap()
            )
        );

        assert_eq!(
            get_buy_shares(
                FixedU64::new_from_int(110, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(11000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap(),
                default_i()
            )
            .unwrap(),
            (
                FixedU64::new_from_int(10, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(110, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(11000, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(110, DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU64::new_from_int(11000, DEFAULT_TOKEN_DECIMALS).unwrap(),
            )
        );

        //  above result is for deposit result
        //  Input
        //  deposit_token_a_amount = 10, deposit_token_b_amount = 1000,
        //  token_a_amount = 100, token_b_amount = 10000, k = 0.5, i = 100,
        //  base_target = 100, base_reserve = 100, quote_target = 10000, quote_reserve = 10000
        //  Result
        //  base_target = 110, base_reserve = 110, quote_target = 11000, quote_reserve = 11000
        //  pool_mint_supply = 110

        assert_eq!(
            general_integrate(q0, q1, q1.checked_sub(delta_b).unwrap(), i, k)
                .unwrap()
                .into_real_u64_ceil(),
            10103
        );

        assert_eq!(
            solve_quadratic_function_for_trade(q0, q1, delta_b, i, k)
                .unwrap()
                .into_real_u64_ceil(),
            3334
        );

        assert_eq!(
            solve_quadratic_function_for_target(q1, delta_b, i, k)
                .unwrap()
                .into_real_u64_ceil(),
            11180
        );
    }
}
