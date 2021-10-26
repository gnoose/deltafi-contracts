//! Calculation functions

use std::cmp::Ordering;

use crate::{
    error::SwapError,
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
};
use solana_program::program_error::ProgramError;

/// Integrate dodo curve from V1 to V2
/// require V0>=V1>=V2>0
/// res = (1-k)i(V1-V2)+ikV0*V0(1/V2-1-V1)
/// let V1-V2=delta
/// res = i*delta*(1-k+k(V0^2/V1/V2))
/// support k=1 & k=0 case
pub fn general_integrate(
    v0: Decimal,
    v1: Decimal,
    v2: Decimal,
    i: Decimal,
    k: Decimal,
) -> Result<Decimal, ProgramError> {
    let fair_amount = v1.try_sub(v2)?.try_mul(i)?; // i*delta
    if k.is_zero() {
        return Ok(fair_amount);
    }
    let v0_v0_v1_v2 = v0.try_mul(v0)?.try_div(v1)?.try_div(v2)?;
    let penalty = v0_v0_v1_v2.try_mul(k)?; // k(V0^2/V1/V2)
    fair_amount.try_mul(penalty.try_add(Decimal::one())?.try_sub(k)?)
}

/// i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
/// Given Q1 and deltaB, solve Q2
/// This is a quadratic function and the standard version is
/// aQ2^2 + bQ2 + c = 0, where
/// a=1-k
/// -b=(1-k)Q1-kQ0^2/Q1+i*deltaB
/// c=-kQ0^2
/// and Q2=(-b+sqrt(b^2+4(1-k)kQ0^2))/2(1-k)
/// note: another root is negative, abondan
/// if deltaBSig=true, then Q2>Q1
/// if deltaBSig=false, then Q2<Q1
///
/// as we only support sell amount as delta, the deltaB is always negative
/// the input ideltaB is actually -ideltaB in the equation
///
/// support k=1 & k=0 case
pub fn solve_quadratic_for_trade(
    v0: Decimal,
    v1: Decimal,
    delta: Decimal,
    i: Decimal,
    k: Decimal,
) -> Result<Decimal, ProgramError> {
    if v0.is_zero() {
        return Err(SwapError::CalculationFailure.into());
    }
    if delta.is_zero() {
        return Ok(Decimal::zero());
    }

    let idelta = delta.try_mul(i)?;
    if k.is_zero() {
        return Ok(idelta.min(v1));
    }

    if k == Decimal::one() {
        // if k==1
        // Q2=Q1/(1+ideltaBQ1/Q0/Q0)
        // temp = ideltaBQ1/Q0/Q0
        // Q2 = Q1/(1+temp)
        // Q1-Q2 = Q1*(1-1/(1+temp)) = Q1*(temp/(1+temp))
        let temp = idelta.try_mul(v1)?.try_div(v0)?.try_div(v0)?;
        return v1.try_mul(temp)?.try_div(temp.try_add(Decimal::one())?);
    }
    // calculate -b value and sig
    // -b=(1-k)Q1-kQ0^2/Q1+i*deltaB
    // part1 = (1-k)Q1 >=0
    // part2 = kQ0^2/Q1-i*deltaB >=0
    // bAbs = abs(part1-part2)
    // if part1>part2 => b is negative => bSig is false
    // if part2>part1 => b is positive => bSig is true
    let k_q2_q1 = v0.try_mul(v0)?.try_div(v1)?.try_mul(k)?.try_add(idelta)?; // kQ0^2/Q1-i*deltaB
    let mut b = Decimal::one().try_sub(k)?.try_mul(v1)?; // (1-k)Q1

    let b_sig = if b < k_q2_q1 {
        b = k_q2_q1.try_sub(b)?;
        true
    } else {
        b = b.try_sub(k_q2_q1)?;
        false
    };

    // calculate sqrt
    let square_root = Decimal::one()
        .try_sub(k)?
        .try_mul(4)?
        .try_mul(k)?
        .try_mul(v0)?
        .try_mul(v0)?; // 4(1-k)kQ0^2
    let square_root = b.try_mul(b)?.try_add(square_root)?.sqrt()?; // sqrt(b*b+4(1-k)kQ0^2)

    let denominator = Decimal::one().try_sub(k)?.try_mul(2)?; // 2(1-k)
    let numerator = if b_sig {
        square_root.try_sub(b)?
    } else {
        b.try_add(square_root)?
    };

    let v2 = numerator.try_div(denominator)?;

    match v2.cmp(&v1) {
        Ordering::Greater => Ok(Decimal::zero()),
        _ => Ok(v1.try_sub(v2)?),
    }
}

/// i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
/// Assume Q2=Q0, Given Q1 and deltaB, solve Q0
///
/// support k=1 & k=0 case
pub fn solve_quadratic_for_target(
    v1: Decimal,
    delta: Decimal,
    i: Decimal,
    k: Decimal,
) -> Result<Decimal, ProgramError> {
    if v1.is_zero() {
        return Ok(Decimal::zero());
    }
    if k.is_zero() {
        return delta.try_mul(i)?.try_add(v1);
    }
    // V0 = V1+V1*(sqrt-1)/2k
    // sqrt = √(1+4kidelta/V1)
    // premium = 1+(sqrt-1)/2k
    let square_root = delta
        .try_mul(i)?
        .try_mul(k)?
        .try_mul(4)?
        .try_div(v1)?
        .try_add(Decimal::one())?
        .sqrt()?;

    let premium = square_root
        .try_sub(Decimal::one())?
        .try_div(2)?
        .try_div(k)?
        .try_add(Decimal::one())?;

    premium.try_mul(v1)
}
