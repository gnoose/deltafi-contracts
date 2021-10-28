//! Approximation calculations

use {
    num_traits::{CheckedShl, CheckedShr, PrimInt},
    std::cmp::Ordering,
};

/// Safe and efficient square root computation function.
///
/// # Arguments
///
/// * radicand - Nubmer to calculate square root.
///
/// # Return value
///
/// None for negative, zero for zero and square root for postive.
pub fn sqrt<T: PrimInt + CheckedShl + CheckedShr>(radicand: T) -> Option<T> {
    match radicand.cmp(&T::zero()) {
        Ordering::Less => return None,             // fail for less than 0
        Ordering::Equal => return Some(T::zero()), // do nothing for 0
        _ => {}
    }

    // Compute bit, the largest power of 4 <= n
    let max_shift: u32 = T::zero().leading_zeros() - 1;
    let shift: u32 = (max_shift - radicand.leading_zeros()) & !1;
    let mut bit = T::one().checked_shl(shift)?;

    let mut n = radicand;
    let mut result = T::zero();
    while bit != T::zero() {
        let result_with_bit = result.checked_add(&bit)?;
        if n >= result_with_bit {
            n = n.checked_sub(&result_with_bit)?;
            result = result.checked_shr(1)?.checked_add(&bit)?;
        } else {
            result = result.checked_shr(1)?;
        }
        bit = bit.checked_shr(2)?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use {super::*, proptest::prelude::*};

    fn check_square_root(radicand: u128) {
        let root = sqrt(radicand).unwrap();
        let lower_bound = root.saturating_sub(1).checked_pow(2).unwrap();
        let upper_bound = root.checked_add(1).unwrap().checked_pow(2).unwrap();
        assert!(radicand as u128 <= upper_bound);
        assert!(radicand as u128 >= lower_bound);
    }

    #[test]
    fn test_square_root_min_max() {
        let test_roots = [0, u64::MAX];
        for i in test_roots.iter() {
            check_square_root(*i as u128);
        }
    }

    #[test]
    fn test_square_root_negative() {
        let neg_num: i128 = -1;
        assert!(sqrt(neg_num).is_none());
    }

    #[test]
    fn test_square_root_exact() {
        let test_nums: [u128; 7] = [0, 1, 4, 5, 9, 34028074089, u128::MAX];
        let test_roots: [u128; 7] = [0, 1, 2, 2, 3, 184467, 18446744073709551615];
        for (idx, test_num) in test_nums.iter().enumerate() {
            assert_eq!(sqrt(*test_num).unwrap() as u128, test_roots[idx]);
        }
    }

    proptest! {
        #[test]
        fn test_square_root(a in 0..u64::MAX) {
            check_square_root(a as u128);
        }
    }
}
