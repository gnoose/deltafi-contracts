//! Implement DODOMath_v2 calculation

use crate::bn::{FixedU256, U256};

/// calculate deposit amount according to the reserve amount
//      a_reserve = 0 & b_reserve = 0 => (a_amount, b_amount)
//      a_reserve > 0 & b_reserve = 0 => (a_amount, 0)
//      a_reserve > 0 & b_reserve > 0 => (a_amount*ratio1, b_amount*ratio2)

pub fn get_deposit_adjustment_amount_v2(
    base_in_amount: FixedU256,
    quote_in_amount: FixedU256,
    base_reserve_amount: FixedU256,
    quote_reserve_amount: FixedU256,
) -> Option<(FixedU256, FixedU256)> {
    if quote_reserve_amount.into_u256_ceil().is_zero()
        && base_reserve_amount.into_u256_ceil().is_zero()
    {
        return Some((base_in_amount, quote_in_amount));
    }

    if quote_reserve_amount.into_u256_ceil().is_zero()
        && base_reserve_amount.into_u256_ceil() > U256::zero()
    {
        return Some((base_in_amount, FixedU256::new(U256::zero().into())));
    }

    if quote_reserve_amount.into_u256_ceil() > U256::zero()
        && base_reserve_amount.into_u256_ceil() > U256::zero()
    {
        let base_increase_ratio = base_in_amount.checked_div_floor(base_reserve_amount)?;
        let quote_increase_ratio = quote_in_amount.checked_div_floor(quote_reserve_amount)?;

        let new_quote_increase_ratio =
            quote_increase_ratio.take_and_scale(base_increase_ratio.base_point)?;
        if base_increase_ratio.inner <= new_quote_increase_ratio.inner {
            Some((
                base_in_amount,
                quote_reserve_amount.checked_mul_floor(base_increase_ratio)?,
            ))
        } else {
            Some((
                base_reserve_amount.checked_mul_floor(quote_increase_ratio)?,
                quote_in_amount,
            ))
        }
    } else {
        Some((base_in_amount, quote_in_amount))
    }
}

/// buy shares [round down] - mint amount for lp
pub fn get_buy_shares_v2(
    base_balance: FixedU256,
    quote_balance: FixedU256,
    base_reserve: FixedU256,
    quote_reserve: FixedU256,
    total_supply: FixedU256,
) -> Option<(FixedU256, FixedU256, FixedU256)> {
    let base_input = base_balance.checked_sub(base_reserve)?;
    let quote_input = quote_balance.checked_sub(quote_reserve)?;

    // Round down when withdrawing. Therefore, never be a situation occuring balance is 0 but totalsupply is not 0
    // But May Happen，reserve >0 But totalSupply = 0
    let mut share = FixedU256::zero();
    if total_supply.into_u256_ceil().is_zero() {
        // case 1. initial supply
        share = base_balance;
    } else if base_reserve.into_u256_ceil() > U256::zero()
        && quote_reserve.into_u256_ceil().is_zero()
    {
        // case 2. supply when quote reserve is 0
        share = base_input
            .checked_mul_floor(total_supply)?
            .checked_div_floor(base_reserve)?;
    } else if base_reserve.into_u256_ceil() > U256::zero()
        && quote_reserve.into_u256_ceil() > U256::zero()
    {
        // case 3. normal case
        let base_input_ratio = base_input.checked_div_floor(base_reserve)?;
        let quote_input_ratio = quote_input.checked_div_floor(quote_reserve)?;
        let mint_ratio;
        let new_quote_input_ratio =
            quote_input_ratio.take_and_scale(base_input_ratio.base_point)?;
        if new_quote_input_ratio.inner < base_input_ratio.inner {
            mint_ratio = quote_input_ratio;
        } else {
            mint_ratio = base_input_ratio;
        }
        share = total_supply.checked_mul_floor(mint_ratio)?;
    }
    Some((share, base_input, quote_input))
}

/// Integrate dodo curve from V1 to V2
//        require V0>=V1>=V2>0
//        res = (1-k)i(V1-V2)+ikV0*V0(1/V2-1/V1)
//        let V1-V2=delta
//        res = i*delta*(1-k+k(V0^2/V1/V2))

//        i is the price of V-res trading pair

//        support k=1 & k=0 case

//        [round down]

pub fn general_integrate_v2(
    v0: FixedU256,
    v1: FixedU256,
    v2: FixedU256,
    i: FixedU256,
    k: FixedU256,
) -> Option<FixedU256> {
    let fair_amount = i.checked_mul_floor(v1.checked_sub(v2)?)?;

    let v0v0v1v2 = v0
        .checked_mul_floor(v0)?
        .checked_div_floor(v1)?
        .checked_div_ceil(v2)?;

    let penalty = k.checked_mul_floor(v0v0v1v2)?; // k(V0^2/V1/V2)

    fair_amount.checked_mul_floor(FixedU256::one().checked_sub(k)?.checked_add(penalty)?)
}

/// Follow the integration function above
//    i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
//    Assume Q2=Q0, Given Q1 and deltaB, solve Q0

//    i is the price of delta-V trading pair
//    give out target of V

//    support k=1 & k=0 case

//    [round down]

pub fn solve_quadratic_function_for_target_v2(
    v1: FixedU256,
    delta: FixedU256,
    i: FixedU256,
    k: FixedU256,
) -> Option<FixedU256> {
    if v1.into_u256_ceil().is_zero() {
        return Some(FixedU256::zero());
    }

    if k.into_u256_ceil().is_zero() {
        return v1.checked_add(i.checked_mul_floor(delta)?);
    }

    // V0 = V1*(1+(sqrt-1)/2k)
    // sqrt = √(1+4kidelta/V1)
    // premium = 1+(sqrt-1)/2k
    // uint256 sqrt = (4 * k).mul(i).mul(delta).div(V1).add(DecimalMath.ONE2).sqrt();

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
        sqrt =ki
            .checked_div_floor(v1)?
            .checked_mul_floor(delta)?
            .checked_add(FixedU256::one())?
            .sqrt()?;
    }

    let premium = sqrt
        .checked_sub(FixedU256::one())?
        .checked_mul_floor(
            k.checked_mul_floor(FixedU256::new(2.into()))?
        )?
        .checked_add(FixedU256::one())?;

    v1.checked_mul_floor(premium)
}

/// Follow the integration expression above, we have:
//        i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
//        Given Q1 and deltaB, solve Q2
//        This is a quadratic function and the standard version is
//        aQ2^2 + bQ2 + c = 0, where
//        a=1-k
//        -b=(1-k)Q1-kQ0^2/Q1+i*deltaB
//        c=-kQ0^2
//        and Q2=(-b+sqrt(b^2+4(1-k)kQ0^2))/2(1-k)
//        note: another root is negative, abondan
//
//        if deltaBSig=true, then Q2>Q1, user sell Q and receive B
//        if deltaBSig=false, then Q2<Q1, user sell B and receive Q
//        return |Q1-Q2|
//
//        as we only support sell amount as delta, the deltaB is always negative
//        the input ideltaB is actually -ideltaB in the equation
//
//        i is the price of delta-V trading pair
//
//        support k=1 & k=0 case
//
//        [round down]

pub fn solve_quadratic_function_for_trade_v2(
    v0: FixedU256,
    v1: FixedU256,
    delta: FixedU256,
    i: FixedU256,
    k: FixedU256,
) -> Option<FixedU256> {

    if v0.into_u256_ceil() <= U256::zero() {
        return Some(FixedU256::zero());
    }

    if delta.into_u256_ceil().is_zero() {
        return Some(FixedU256::zero());
    }

    if k.into_u256_ceil().is_zero() {
        if i.checked_mul_floor(delta)?.into_u256_ceil() > v1.into_u256_ceil() {
            return Some(v1);
        } else {
            return i.checked_mul_floor(delta);
        }
    }

    if k.into_u256_ceil() == U256::one() {
        // if k==1
        // Q2=Q1/(1+ideltaBQ1/Q0/Q0)
        // temp = ideltaBQ1/Q0/Q0
        // Q2 = Q1/(1+temp)
        // Q1-Q2 = Q1*(1-1/(1+temp)) = Q1*(temp/(1+temp))
        // uint256 temp = i.mul(delta).mul(V1).div(V0.mul(V0));
        let temp;
        let i_delta = i.checked_mul_floor(delta)?;
        if i_delta.into_u256_ceil().is_zero() {
            temp = FixedU256::zero();
        } else if i_delta.checked_mul_floor(v1)?.checked_div_floor(i_delta)?.into_u256_ceil() == v1.into_u256_ceil() {
            temp = i_delta
                .checked_mul_floor(v1)?
                .checked_div_floor(
                    v0.checked_mul_floor(v0)?
                )?;
        } else {
            temp = delta
                .checked_mul_floor(v1)?
                .checked_div_floor(v0)?
                .checked_mul_floor(i)?
                .checked_div_floor(v0)?;
        }
        return v1
            .checked_mul_floor(temp)?
            .checked_div_floor(
                temp.checked_add(FixedU256::one())?
            );
    }

    // calculate -b value and sig
    // b = kQ0^2/Q1-i*deltaB-(1-k)Q1
    // part1 = (1-k)Q1 >=0
    // part2 = kQ0^2/Q1-i*deltaB >=0
    // bAbs = abs(part1-part2)
    // if part1>part2 => b is negative => bSig is false
    // if part2>part1 => b is positive => bSig is true

    let part_2 = k
        .checked_mul_floor(v0)?
        .checked_div_floor(v1)?
        .checked_mul_floor(v0)?
        .checked_add(
            i.checked_mul_floor(delta)?
        )?; // kQ0^2/Q1-i*deltaB

    let mut b_abs = FixedU256::one()
        .checked_sub(k)?
        .checked_mul_floor(v1)?; // (1-k)Q1

    let b_sig;
    if b_abs >= part_2 {
        b_abs = b_abs.checked_sub(part_2)?;
        b_sig = false;
    } else {
        b_abs = part_2.checked_sub(b_abs)?;
        b_sig = true;
    }

    b_abs = b_abs.checked_div_floor(FixedU256::one())?;

    // calculate sqrt

    let mut square_root = FixedU256::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU256::new(4.into()))?
        .checked_mul_floor(
            k.checked_mul_floor(v0)?
                .checked_mul_floor(v0)?
        )?; // 4(1-k)kQ0^2

    square_root = b_abs
        .checked_mul_floor(b_abs)?
        .checked_add(square_root)?
        .sqrt()?; // sqrt(b*b+4(1-k)kQ0*Q0)

    // final res

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
        Some(FixedU256::zero())
    } else {
        v1.checked_sub(v2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let q0: FixedU256 = FixedU256::new_from_int(1000.into(), 18).unwrap();
        let q1: FixedU256 = FixedU256::new_from_int(1000.into(), 18).unwrap();
        let i: FixedU256 = FixedU256::new_from_int(100.into(), 18).unwrap();
        let delta_b: FixedU256 = FixedU256::new_from_int(200.into(), 18).unwrap();
        let k: FixedU256 = FixedU256::one()
            .checked_mul_floor(FixedU256::new(5.into()))
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();

        assert_eq!(
            get_buy_shares_v2(
                FixedU256::new_from_int(1000.into(), 18).unwrap(),
                FixedU256::new_from_int(2000.into(), 18).unwrap(),
                FixedU256::new_from_int(800.into(), 18).unwrap(),
                FixedU256::new_from_int(1600.into(), 18).unwrap(),
                FixedU256::new_from_int(10000.into(), 18).unwrap(),
            )
                .unwrap(),
            (
                FixedU256::new_from_int(2500.into(), 18).unwrap(),
                FixedU256::new_from_int(200.into(), 18).unwrap(),
                FixedU256::new_from_int(400.into(), 18).unwrap()
            )
        );

        assert_eq!(
            get_deposit_adjustment_amount_v2(
                FixedU256::new_from_int(10.into(), 18).unwrap(),
                FixedU256::new_from_int(20.into(), 18).unwrap(),
                FixedU256::new_from_int(500.into(), 18).unwrap(),
                FixedU256::new_from_int(2000.into(), 18).unwrap(),
            )
                .unwrap(),
            (
                FixedU256::new_from_int(5.into(), 18).unwrap(),
                FixedU256::new_from_int(20.into(), 18).unwrap()
            )
        );

        assert_eq!(
            general_integrate_v2(q0, q1, q1.checked_sub(delta_b).unwrap(), i, k).unwrap(),
            FixedU256::new_from_int(30000.into(), 18).unwrap()
        );

        assert_eq!(
            solve_quadratic_function_for_trade_v2(q0, q1, delta_b, i, k).unwrap(),
            FixedU256::new_from_int(952.into(), 18).unwrap()
        );

        let value = FixedU256::new(64031.into())
            .checked_mul_floor(FixedU256::new(100000.into()))
            .unwrap()
            .checked_add(FixedU256::new(24237.into())).unwrap()
            .checked_mul_floor(FixedU256::new(1000000.into()))
            .unwrap()
            .checked_mul_floor(FixedU256::new(1000000.into()))
            .unwrap();
        assert_eq!(
            solve_quadratic_function_for_target_v2(q1, delta_b, i, k).unwrap(),
            FixedU256::new_from_fixed(value.into_u256_ceil(), 18)
        );
    }
}
