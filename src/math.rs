//! Implement DODOMath calculation

use crate::bn::U256;

/// calculate sqrt
pub fn integer_sqrt(x: U256) -> U256 {
    let two: U256 = 2.into();

    let mut z: U256 = (x + U256::one()) / two;

    let mut y = x;

    while z < y {
        y = z;
        z = (x / z + z) / two;
    }

    y
}

/// calculate div ceil
pub fn div_ceil(
    value0: U256,
    value1: U256
) -> U256 {
    let result = value0 / value1;
    if value0 - result * value1 > U256::from(0) {
        result + U256::from(1)
    } else {
        result
    }
}

/// calculate div floor
pub fn div_floor(
    value0: U256,
    value1: U256
) -> U256 {
    value0 / value1
}

/// calculate mul ceil
pub fn mul_ceil(
    value0: U256,
    value1: U256
) -> U256 {
    value0 * value1
}

/// calculate mul floor
pub fn mul_floor(
    value0: U256,
    value1: U256
) -> U256 {
    value0 * value1
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
) -> U256 {
    let fair_amount = i * (v1 - v2);
    let v0v0v1v2 = v0 * v0 / v1 / v2;
    let penalty = k * v0v0v1v2;

    fair_amount * (U256::from(1) - k + penalty)
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
) -> U256 {
    // calculate -b value and sig
    // -b = (1-k)Q1-kQ0^2/Q1+i*deltaB
    let mut kq02q1 = k * q0 * q0 / q1;
    let mut b = (U256::from(1) - k) * q1;

    let minus_b_sig;
    if delta_b_sig {
        b = b + i_delta_b;
    } else {
        kq02q1 = kq02q1 + i_delta_b;
    }

    if b >= kq02q1 {
        b = b - kq02q1;
        minus_b_sig = true;
    } else {
        b = kq02q1 - b;
        minus_b_sig = false;
    }

    // calculate sqrt
    let mut square_root = U256::from(4) * (U256::from(1) - k) * k * q0 * q0;
    square_root = integer_sqrt(b  * b + square_root);

    // final res
    let denominator = U256::from(2) * (U256::from(1) - k);
    let numerator;
    if minus_b_sig {
        numerator = b + square_root;
    } else {
        numerator = square_root - b;
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
) -> U256 {
    // V0 = V1+V1*(sqrt-1)/2k
    let mut sqrt = div_ceil(k * fair_amount * U256::from(4), v1);
    sqrt = integer_sqrt((sqrt + U256::from(1)) * U256::from(1));

    let premium = div_ceil(sqrt - U256::from(1), k * U256::from(2));

    // V0 is greater than or equal to V1 according to the solution
    v1 * (U256::from(1) + premium)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    /* uses */
    /// zero value
    pub const ZERO_V: f64 = 0 as f64;
    pub const MAX_V: f64 = f64::MAX;

    #[test]
    fn test_general_integrate() {
        let v0 = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let v1 = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let v2 = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let i = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let k = rand::thread_rng().gen_range(ZERO_V, MAX_V);

        let expected = i * (v1 - v2) * (f64::from(1) - k + k * (v0 * v0 / v1 / v2));

        assert_eq!(
            general_integrate(v0, v1, v2, i, k),
            expected
        );
    }

    #[test]
    fn test_solve_quadratic_function_for_trade() {
        let q0 = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let q1 = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let i_delta_b = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let delta_b_sig = rand::random();
        let k = rand::thread_rng().gen_range(ZERO_V, MAX_V);

        let mut kq02q1 = k * q0 * q0 / q1;
        let mut b = (f64::from(1) - k) * q1;

        let minus_b_sig;
        if delta_b_sig {
            b = b + i_delta_b;
        } else {
            kq02q1 = kq02q1 + i_delta_b;
        }

        if b >= kq02q1 {
            b = b - kq02q1;
            minus_b_sig = true;
        } else {
            b = kq02q1 - b;
            minus_b_sig = false;
        }

        // calculate sqrt
        let mut square_root = f64::from(4) * (f64::from(1) - k) * k * q0 * q0;
        square_root = (b  * b + square_root).sqrt();

        // final res
        let denominator = f64::from(2) * (f64::from(1) - k);
        let numerator;
        if minus_b_sig {
            numerator = b + square_root;
        } else {
            numerator = square_root - b;
        }

        let result = numerator / denominator;
        let expected = result;
        if delta_b_sig {
            expected = result.floor();
        } else {
            expected = result.ceil();
        }

        assert_eq!(
            solve_quadratic_function_for_trade(q0, q1, i_delta_b, delta_b_sig, k),
            expected
        );
    }

    #[test]
    fn test_solve_quadratic_function_for_target() {
        let v1 = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let k = rand::thread_rng().gen_range(ZERO_V, MAX_V);
        let fair_amount = rand::thread_rng().gen_range(ZERO_V, MAX_V);

        let mut sqrt = (k * fair_amount * f64::from(4) / v1).ceil();
        sqrt = ((sqrt + f64::from(1)) * f64::from(1)).sqrt();

        let expected = v1 + v1 * (sqrt - f64::from(1)) / f64::from(2) / k;

        assert_eq!(
            solve_quadratic_function_for_target(v1, k, fair_amount),
            expected
        );
    }
}