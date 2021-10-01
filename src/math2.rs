//! math formulas for proactive market maker
use solana_program::program_error::ProgramError;

use crate::bn::{FixedU256, U256};

/// calculate deposit amount according to the reserve amount
//      a_reserve = 0 & b_reserve = 0 => (a_amount, b_amount)
//      a_reserve > 0 & b_reserve = 0 => (a_amount, 0)
//      a_reserve > 0 & b_reserve > 0 => (a_amount*ratio1, b_amount*ratio2)
pub fn get_deposit_adjustment_amount(
    base_in_amount: FixedU256,
    quote_in_amount: FixedU256,
    base_reserve_amount: FixedU256,
    quote_reserve_amount: FixedU256,
    i: FixedU256,
) -> Result<(FixedU256, FixedU256), ProgramError> {
    if quote_reserve_amount.into_u256_ceil().is_zero()
        && base_reserve_amount.into_u256_ceil().is_zero()
    {
        let shares;
        if quote_in_amount.into_u256_ceil() < base_in_amount.checked_mul_floor(i)?.into_u256_ceil()
        {
            shares = quote_in_amount.checked_div_floor(i)?;
        } else {
            shares = base_in_amount;
        }
        let base_adjusted_in_amount = shares;
        let quote_adjusted_in_amount = shares.checked_mul_floor(i)?;

        return Ok((base_adjusted_in_amount, quote_adjusted_in_amount));
    }

    if quote_reserve_amount.into_u256_ceil() > U256::zero()
        && base_reserve_amount.into_u256_ceil() > U256::zero()
    {
        let base_increase_ratio = base_in_amount.checked_div_floor(base_reserve_amount)?;
        let quote_increase_ratio = quote_in_amount.checked_div_floor(quote_reserve_amount)?;

        let new_quote_increase_ratio =
            quote_increase_ratio.take_and_scale(base_increase_ratio.base_point())?;
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
    base_balance: FixedU256,
    quote_balance: FixedU256,
    base_reserve: FixedU256,
    quote_reserve: FixedU256,
    base_target: FixedU256,
    quote_target: FixedU256,
    total_supply: FixedU256,
    i: FixedU256,
) -> Result<(FixedU256, FixedU256, FixedU256, FixedU256, FixedU256), ProgramError> {
    let base_input = base_balance.checked_sub(base_reserve)?;
    let quote_input = quote_balance.checked_sub(quote_reserve)?;

    if base_input.into_u256_ceil() <= U256::zero() {
        return Err(ProgramError::InvalidArgument);
    }

    let mut share = FixedU256::zero();
    let mut new_base_target = base_target;
    let mut new_quote_target = quote_target;
    if total_supply.into_u256_ceil().is_zero() {
        // case 1. initial supply
        if quote_balance.into_u256_ceil() < base_balance.checked_mul_floor(i)?.into_u256_ceil() {
            share = quote_balance.checked_div_floor(i)?;
        } else {
            share = base_balance;
        }
        new_base_target = share;
        new_quote_target = share.checked_mul_floor(i)?;
    } else if base_reserve.into_u256_ceil() > U256::zero()
        && quote_reserve.into_u256_ceil() > U256::zero()
    {
        let base_input_ratio = base_input.checked_div_floor(base_reserve)?;
        let quote_input_ratio = quote_input.checked_div_floor(quote_reserve)?;
        let mint_ratio;
        let new_quote_input_ratio =
            quote_input_ratio.take_and_scale(base_input_ratio.base_point())?;
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
    v0: FixedU256,
    v1: FixedU256,
    v2: FixedU256,
    i: FixedU256,
    k: FixedU256,
) -> Result<FixedU256, ProgramError> {
    let fair_amount = i.checked_mul_floor(v1.checked_sub(v2)?)?;

    let v0v0v1v2 = v0
        .checked_mul_floor(v0)?
        .checked_div_floor(v1)?
        .checked_div_ceil(v2)?;

    let penalty = k.checked_mul_floor(v0v0v1v2)?; // k(V0^2/V1/V2)

    fair_amount.checked_mul_floor(FixedU256::one().checked_sub(k)?.checked_add(penalty)?)
}

/// Follow the integration function above
pub fn solve_quadratic_function_for_target(
    v1: FixedU256,
    delta: FixedU256,
    i: FixedU256,
    k: FixedU256,
) -> Result<FixedU256, ProgramError> {
    if v1.into_u256_ceil().is_zero() {
        return Ok(FixedU256::zero());
    }

    if k.into_u256_ceil().is_zero() {
        return v1.checked_add(i.checked_mul_floor(delta)?);
    }

    let sqrt;
    let ki = k
        .checked_mul_floor(FixedU256::new(4.into()))?
        .checked_mul_floor(i)?;

    if ki.into_u256_ceil().is_zero() {
        sqrt = FixedU256::one();
    } else if ki.checked_mul_floor(delta)?.checked_div_floor(ki)? == delta {
        sqrt = ki
            .checked_mul_floor(delta)?
            .checked_div_floor(v1)?
            .checked_add(FixedU256::one())?
            .sqrt()?;
    } else {
        sqrt = ki
            .checked_div_floor(v1)?
            .checked_mul_floor(delta)?
            .checked_add(FixedU256::one())?
            .sqrt()?;
    }

    let premium = sqrt
        .checked_sub(FixedU256::one())?
        .checked_div_floor(k.checked_mul_floor(FixedU256::new(2.into()))?)?
        .checked_add(FixedU256::one())?;

    v1.checked_mul_floor(premium)
}

/// Follow the integration expression above
pub fn solve_quadratic_function_for_trade(
    v0: FixedU256,
    v1: FixedU256,
    delta: FixedU256,
    i: FixedU256,
    k: FixedU256,
) -> Result<FixedU256, ProgramError> {
    if v0.into_u256_ceil() <= U256::zero() {
        return Ok(FixedU256::zero());
    }

    if delta.into_u256_ceil().is_zero() {
        return Ok(FixedU256::zero());
    }

    if k.into_u256_ceil().is_zero() {
        if i.checked_mul_floor(delta)?.into_u256_ceil() > v1.into_u256_ceil() {
            return Ok(v1);
        } else {
            return i.checked_mul_floor(delta);
        }
    }

    if k.into_u256_ceil() == U256::one() {
        let temp;
        let i_delta = i.checked_mul_floor(delta)?;
        if i_delta.into_u256_ceil().is_zero() {
            temp = FixedU256::zero();
        } else if i_delta
            .checked_mul_floor(v1)?
            .checked_div_floor(i_delta)?
            .into_u256_ceil()
            == v1.into_u256_ceil()
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
            .checked_div_floor(temp.checked_add(FixedU256::one())?);
    }

    let part_2 = k
        .checked_mul_floor(v0)?
        .checked_div_floor(v1)?
        .checked_mul_floor(v0)?
        .checked_add(i.checked_mul_floor(delta)?)?; // kQ0^2/Q1-i*deltaB

    let mut b_abs = FixedU256::one().checked_sub(k)?.checked_mul_floor(v1)?; // (1-k)Q1

    let b_sig;
    if b_abs >= part_2 {
        b_abs = b_abs.checked_sub(part_2)?;
        b_sig = false;
    } else {
        b_abs = part_2.checked_sub(b_abs)?;
        b_sig = true;
    }

    b_abs = b_abs.checked_div_floor(FixedU256::one())?;

    let mut square_root = FixedU256::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU256::new(4.into()))?
        .checked_mul_floor(k.checked_mul_floor(v0)?.checked_mul_floor(v0)?)?; // 4(1-k)kQ0^2

    square_root = b_abs
        .checked_mul_floor(b_abs)?
        .checked_add(square_root)?
        .sqrt()?;

    let denominator = FixedU256::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU256::new(2.into()))?; // 2(1-k)
    let numerator;

    if b_sig {
        numerator = square_root.checked_sub(b_abs)?;
    } else {
        numerator = b_abs.checked_add(square_root)?;
    }

    let v2 = numerator.checked_div_ceil(denominator)?;
    if v2.into_u256_ceil() > v1.into_u256_ceil() {
        Ok(FixedU256::zero())
    } else {
        Ok(v1.checked_sub(v2)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{
        test_utils::{default_i, default_k},
        DEFAULT_TOKEN_DECIMALS,
    };

    #[test]
    fn basic() {
        let q0: FixedU256 = FixedU256::new_from_int(5000.into(), DEFAULT_TOKEN_DECIMALS).unwrap();
        let q1: FixedU256 = FixedU256::new_from_int(5000.into(), DEFAULT_TOKEN_DECIMALS).unwrap();
        let i: FixedU256 = default_i();
        let delta_b: FixedU256 =
            FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap();
        let k: FixedU256 = default_k();

        assert_eq!(
            get_deposit_adjustment_amount(
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(0.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(0.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                i
            )
            .unwrap(),
            (
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap()
            )
        );

        assert_eq!(
            get_buy_shares(
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(0.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(0.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::zero(),
                FixedU256::zero(),
                FixedU256::zero(),
                default_i()
            )
            .unwrap(),
            (
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
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
                FixedU256::new_from_int(10.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(1000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                i
            )
            .unwrap(),
            (
                FixedU256::new_from_int(10.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(1000.into(), DEFAULT_TOKEN_DECIMALS).unwrap()
            )
        );

        assert_eq!(
            get_buy_shares(
                FixedU256::new_from_int(110.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(11000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                default_i()
            )
            .unwrap(),
            (
                FixedU256::new_from_int(10.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(110.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(11000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(110.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
                FixedU256::new_from_int(11000.into(), DEFAULT_TOKEN_DECIMALS).unwrap(),
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
            U256::to_u64(
                general_integrate(q0, q1, q1.checked_sub(delta_b).unwrap(), i, k)
                    .unwrap()
                    .into_u256_ceil()
            )
            .unwrap(),
            15000
        );

        assert_eq!(
            U256::to_u64(
                solve_quadratic_function_for_trade(q0, q1, delta_b, i, k)
                    .unwrap()
                    .into_u256_ceil()
            )
            .unwrap(),
            3333
        );

        assert_eq!(
            U256::to_u64(
                solve_quadratic_function_for_target(q1, delta_b, i, k)
                    .unwrap()
                    .into_u256_ceil()
            )
            .unwrap(),
            10000
        );
    }
}
