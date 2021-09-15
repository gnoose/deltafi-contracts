//! Implement DODOMath calculation

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
) -> Option<(FixedU256, FixedU256)> {
    if quote_reserve_amount.into_u256_ceil().is_zero()
        && base_reserve_amount.into_u256_ceil().is_zero()
    {
        return Some((base_in_amount, quote_in_amount))
    }

    if quote_reserve_amount.into_u256_ceil().is_zero()
        && base_reserve_amount.into_u256_ceil() > U256::zero()
    {
        return Some((base_in_amount, FixedU256::new(U256::zero().into())?))
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

/// integrate curve from v1 to v2  = i * delta * (1 - k + k(v0^2 / v1 /v2))
//         require V0>=V1>=V2>0
//         res = (1-k)i(V1-V2)+ikV0*V0(1/V2-1/V1)
//         let V1-V2=delta
//         res = i*delta*(1-k+k(V0^2/V1/V2))
pub fn general_integrate(
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

    let penalty = k.checked_mul_floor(v0v0v1v2)?;

    fair_amount.checked_mul_floor(FixedU256::one().checked_sub(k)?.checked_add(penalty)?)
}

/// The same with integration expression above, we have:
//         i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
//         Given Q1 and deltaB, solve Q2
//         This is a quadratic function and the standard version is
//         aQ2^2 + bQ2 + c = 0, where
//         a=1-k
//         -b=(1-k)Q1-kQ0^2/Q1+i*deltaB
//         c=-kQ0^2
//         and Q2=(-b+sqrt(b^2+4(1-k)kQ0^2))/2(1-k)
//         note: another root is negative, abondan
//         if deltaBSig=true, then Q2>Q1
//         if deltaBSig=false, then Q2<Q1
pub fn solve_quadratic_function_for_trade(
    q0: FixedU256,
    q1: FixedU256,
    i_delta_b: FixedU256,
    delta_b_sig: bool,
    k: FixedU256,
) -> Option<FixedU256> {
    // calculate -b value and sig
    // -b = (1-k)Q1-kQ0^2/Q1+i*deltaB
    let mut kq02q1 = k
        .checked_mul_floor(q0)?
        .checked_mul_floor(q0)?
        .checked_div_floor(q1)?;
    let mut b = FixedU256::one().checked_sub(k)?.checked_mul_floor(q1)?;

    if delta_b_sig {
        b = b.checked_add(i_delta_b)?;
    } else {
        kq02q1 = kq02q1.checked_add(i_delta_b)?;
    }

    let minus_b_sig;
    if b.into_u256_ceil() >= kq02q1.into_u256_ceil() {
        b = b.checked_sub(kq02q1)?;
        minus_b_sig = true;
    } else {
        b = kq02q1.checked_sub(b)?;
        minus_b_sig = false;
    }
    //
    // // calculate sqrt
    let mut square_root = FixedU256::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU256::new(4.into())?)?
        .checked_mul_floor(k)?
        .checked_mul_floor(q0)?
        .checked_mul_floor(q0)?;
    square_root = b.checked_mul_floor(b)?.checked_add(square_root)?.sqrt()?;

    // final res
    let denominator = FixedU256::one()
        .checked_sub(k)?
        .checked_mul_floor(FixedU256::new(2.into())?)?;

    let numerator;
    if minus_b_sig {
        numerator = b.checked_add(square_root)?;
    } else {
        numerator = square_root.checked_sub(b)?;
    }

    if delta_b_sig {
        numerator.checked_div_floor(denominator)
    } else {
        numerator.checked_div_ceil(denominator)
    }
}

/// Start from the integration function
//         i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
//         Assume Q2=Q0, Given Q1 and deltaB, solve Q0
//         let fairAmount = i*deltaB
pub fn solve_quadratic_function_for_target(
    v1: FixedU256,
    k: FixedU256,
    fair_amount: FixedU256,
) -> Option<FixedU256> {
    // V0 = V1+V1*(sqrt-1)/2k
    let mut sqrt = k
        .checked_mul_floor(fair_amount)?
        .checked_mul_floor(FixedU256::new(4.into())?)?
        .checked_div_ceil(v1)?;
    sqrt = sqrt
        .checked_add(FixedU256::one())?
        .checked_mul_floor(FixedU256::one())?
        .sqrt()?;

    let premium = sqrt
        .checked_sub(FixedU256::one())?
        .checked_div_ceil(k.checked_mul_floor(FixedU256::new(2.into())?)?)?;

    // V0 is greater than or equal to V1 according to the solution
    v1.checked_mul_floor(premium.checked_add(FixedU256::one())?)
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
        let i_delta_b: FixedU256 = i.checked_mul_floor(delta_b).unwrap();
        let k: FixedU256 = FixedU256::one()
            .checked_mul_floor(FixedU256::new(5.into()).unwrap())
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()).unwrap())
            .unwrap();

        assert_eq!(
            get_deposit_adjustment_amount(
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
            general_integrate(q0, q1, q1.checked_sub(delta_b).unwrap(), i, k).unwrap(),
            FixedU256::new_from_int(30000.into(), 18).unwrap()
        );

        assert_eq!(
            solve_quadratic_function_for_trade(q0, q1, i_delta_b, false, k).unwrap(),
            FixedU256::new_from_int(25.into(), 18).unwrap()
        );

        assert_eq!(
            solve_quadratic_function_for_target(q0, k, i_delta_b).unwrap(),
            FixedU256::new_from_int(7000.into(), 18).unwrap()
        );
    }
}
