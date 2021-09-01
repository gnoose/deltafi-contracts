//! Implement DODOMath calculation

/// integrate curve from v1 to v2  = i * delta * (1 - k + k(v0^2 / v1 /v2))
//         require V0>=V1>=V2>0
//         res = (1-k)i(V1-V2)+ikV0*V0(1/V2-1/V1)
//         let V1-V2=delta
//         res = i*delta*(1-k+k(V0^2/V1/V2))
pub fn general_integrate(
    v0: f64,
    v1: f64,
    v2: f64,
    i: f64,
    k: f64
) -> f64 {
    let fair_amount = i * (v1 - v2);
    let v0v0v1v2 = v0 * v0 / v1 / v2;
    let penalty = k * v0v0v1v2;

    fair_amount * (f64::from(1) - k + penalty)
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
    q0: f64,
    q1: f64,
    i_delta_b: f64,
    delta_b_sig: bool,
    k: f64
) -> f64 {
    // calculate -b value and sig
    // -b = (1-k)Q1-kQ0^2/Q1+i*deltaB
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
    if delta_b_sig {
        result.floor()
    } else {
        result.ceil()
    }
}

/// Start from the integration function
//         i*deltaB = (Q2-Q1)*(1-k+kQ0^2/Q1/Q2)
//         Assume Q2=Q0, Given Q1 and deltaB, solve Q0
//         let fairAmount = i*deltaB
pub fn solve_quadratic_function_for_target (
    v1: f64,
    k: f64,
    fair_amount: f64
) -> f64 {
    // V0 = V1+V1*(sqrt-1)/2k
    let mut sqrt = (k * fair_amount * f64::from(4) / v1).ceil();
    sqrt = ((sqrt + f64::from(1)) * f64::from(1)).sqrt();

    let premium = ((sqrt - f64::from(1)) / (k * f64::from(1))).ceil();

    // V0 is greater than or equal to V1 according to the solution
    v1 * (f64::from(1) + premium)
}