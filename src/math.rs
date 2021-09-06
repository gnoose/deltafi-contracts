//! Implement DODOMath calculation

use crate::bn::U256;

/// calculate sqrt
pub fn integer_sqrt(x: U256) -> Option<U256> {
    let two: U256 = 2.into();

    let mut z = x.checked_add(U256::one())?.checked_div(two)?;

    let mut y = x;

    while z < y {
        y = z;
        z = x.checked_div(z)?.checked_add(z)?.checked_div(two)?;
    }

    Some(y)
}

/// calculate div ceil
pub fn div_ceil(
    value0: U256,
    value1: U256
) -> Option<U256> {
    let result = value0.checked_div(value1)?;

    if value0.checked_sub(result.checked_mul(value1)?)? > 0.into() {
        result.checked_add(U256::one())
    } else {
        Some(result)
    }
}

/// calculate div floor
pub fn div_floor(
    value0: U256,
    value1: U256
) -> Option<U256> {
    value0.checked_div(value1)
}

/// calculate mul ceil
pub fn mul_ceil(
    value0: U256,
    value1: U256
) -> Option<U256> {
    value0.checked_mul(value1)
}

/// calculate mul floor
pub fn mul_floor(
    value0: U256,
    value1: U256
) -> Option<U256> {
    value0.checked_mul(value1)
}

/// integrate curve from v1 to v2  = i * delta * (1 - k + k(v0^2 / v1 /v2))
//         require V0>=V1>=V2>0
//         res = (1-k)i(V1-V2)+ikV0*V0(1/V2-1/V1)
//         let V1-V2=delta
//         res = i*delta*(1-k+k(V0^2/V1/V2))
pub fn general_integrate(
    v0: U256,
    v1: U256,
    v2: U256,
    i: U256,
    k: U256
) -> Option<U256> {
    let fair_amount = i.checked_mul(v1.checked_sub(v2)?)?;

    let v0v0v1v2 = v0.checked_mul(v0)?.checked_div(v1)?.checked_div(v2)?;

    let penalty = k.checked_mul(v0v0v1v2)?;

    fair_amount.checked_mul(U256::one().checked_sub(k)?.checked_add(penalty)?)
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
    q0: U256,
    q1: U256,
    i_delta_b: U256,
    delta_b_sig: bool,
    k: U256
) -> Option<U256> {
    // calculate -b value and sig
    // -b = (1-k)Q1-kQ0^2/Q1+i*deltaB
    let mut kq02q1 = k.checked_mul(q0)?.checked_mul(q0)?.checked_div(q1)?;
    let mut b = U256::one().checked_sub(k)?.checked_mul(q1)?;

    let minus_b_sig;
    if delta_b_sig {
        b = b.checked_add(i_delta_b)?;
    } else {
        kq02q1 = kq02q1.checked_add(i_delta_b)?;
    }

    if b >= kq02q1 {
        b = b.checked_sub(kq02q1)?;
        minus_b_sig = true;
    } else {
        b = kq02q1.checked_sub(b)?;
        minus_b_sig = false;
    }

    // calculate sqrt
    let mut square_root = U256::one().checked_sub(k)?.checked_mul(4.into())?.checked_mul(k)?.checked_mul(q0)?.checked_mul(q0)?;
    square_root = integer_sqrt(b.checked_mul(b)?.checked_add(square_root)?)?;

    // final res
    let denominator = U256::one().checked_sub(k)?.checked_mul(2.into())?;
    let numerator;
    if minus_b_sig {
        numerator = b.checked_add(square_root)?;
    } else {
        numerator = square_root.checked_sub(b)?;
    }

    if delta_b_sig {
        div_floor(numerator, denominator)
    } else {
        div_ceil(numerator, denominator)
    }
}

/// Start from the integration function
//         i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
//         Assume Q2=Q0, Given Q1 and deltaB, solve Q0
//         let fairAmount = i*deltaB
pub fn solve_quadratic_function_for_target (
    v1: U256,
    k: U256,
    fair_amount: U256
) -> Option<U256> {
    // V0 = V1+V1*(sqrt-1)/2k
    let mut sqrt = div_ceil(k.checked_mul(fair_amount)?.checked_mul(4.into())?, v1)?;
    sqrt = integer_sqrt(sqrt.checked_add(U256::one())?.checked_mul(U256::one())?)?;

    let premium = div_ceil(sqrt.checked_sub(U256::one())?, k.checked_mul(2.into())?)?;

    // V0 is greater than or equal to V1 according to the solution
    v1.checked_mul(premium.checked_add(U256::one())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    /* uses */
    /// zero value
    pub const ONE_V: u64 = 1 as u64;
    pub const MAX_V: u64 = u64::MAX;

    #[test]
    fn test_integer_sqrt() {
        let value: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        assert_eq!(
            integer_sqrt(value * value).unwrap(),
            value
        );
    }

    #[test]
    fn test_div_ceil() {
        let value: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        assert_eq!(
            div_ceil(value * value + U256::one(), value).unwrap(),
            value + U256::one()
        );
    }

    #[test]
    fn test_div_floor() {
        let value: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        assert_eq!(
            div_floor(value * value + U256::one(), value).unwrap(),
            value
        );
    }

    #[test]
    fn test_mul_ceil() {
        let value: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        assert_eq!(
            mul_ceil(value, value).unwrap(),
            value * value
        );
    }

    #[test]
    fn test_mul_floor() {
        let value: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        assert_eq!(
            mul_floor(value, value).unwrap(),
            value * value
        );
    }

    #[test]
    fn test_general_integrate() {
        let v0: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let v1: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let v2: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let i: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let k: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        // expected = i * (v1 - v2) * (1 - k + k * (v0 * v0 / v1 / v2));

        let fair_amount = i.checked_mul(v1.checked_sub(v2).unwrap()).unwrap();

        let v0v0v1v2 = v0.checked_mul(v0).unwrap().checked_div(v1).unwrap().checked_div(v2).unwrap();

        let penalty = k.checked_mul(v0v0v1v2).unwrap();

        let expected = fair_amount.checked_mul(U256::one().checked_sub(k).unwrap().checked_add(penalty).unwrap()).unwrap();

        assert_eq!(
            general_integrate(v0, v1, v2, i, k).unwrap(),
            expected
        );
    }

    #[test]
    fn test_solve_quadratic_function_for_trade() {
        let q0: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let q1: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let i_delta_b: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let k: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let delta_b_sig = rand::random();

        let mut kq02q1 = k.checked_mul(q0).unwrap().checked_mul(q0).unwrap().checked_div(q1).unwrap();
        let mut b = U256::one().checked_sub(k).unwrap().checked_mul(q1).unwrap();

        let minus_b_sig;
        if delta_b_sig {
            b = b.checked_add(i_delta_b).unwrap();
        } else {
            kq02q1 = kq02q1.checked_add(i_delta_b).unwrap();
        }

        if b >= kq02q1 {
            b = b.checked_sub(kq02q1).unwrap();
            minus_b_sig = true;
        } else {
            b = kq02q1.checked_sub(b).unwrap();
            minus_b_sig = false;
        }

        // calculate sqrt
        let mut square_root = U256::one().checked_sub(k).unwrap().checked_mul(4.into()).unwrap().checked_mul(k).unwrap().checked_mul(q0).unwrap().checked_mul(q0).unwrap();
        square_root = integer_sqrt(b.checked_mul(b).unwrap().checked_add(square_root).unwrap()).unwrap();

        // final res
        let denominator = U256::one().checked_sub(k).unwrap().checked_mul(2.into()).unwrap();
        let numerator;
        if minus_b_sig {
            numerator = b.checked_add(square_root).unwrap();
        } else {
            numerator = square_root.checked_sub(b).unwrap();
        }

        let expected;
        if delta_b_sig {
            expected = div_floor(numerator, denominator).unwrap();
        } else {
            expected = div_ceil(numerator, denominator).unwrap();
        }

        assert_eq!(
            solve_quadratic_function_for_trade(q0, q1, i_delta_b, delta_b_sig, k).unwrap(),
            expected
        );
    }

    #[test]
    fn test_solve_quadratic_function_for_target() {
        let v1: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let k: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let fair_amount: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let mut sqrt = div_ceil(k.checked_mul(fair_amount).unwrap().checked_mul(4.into()).unwrap(), v1).unwrap();
        sqrt = integer_sqrt(sqrt.checked_add(U256::one()).unwrap().checked_mul(U256::one()).unwrap()).unwrap();

        let premium = div_ceil(sqrt.checked_sub(U256::one()).unwrap(), k.checked_mul(2.into()).unwrap()).unwrap();

        // V0 is greater than or equal to V1 according to the solution
        let expected = v1.checked_mul(premium.checked_add(U256::one()).unwrap()).unwrap();

        assert_eq!(
            solve_quadratic_function_for_target(v1, k, fair_amount).unwrap(),
            expected
        );
    }
}