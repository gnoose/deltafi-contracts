//! Big number types

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]

use std::convert::TryInto;

use uint::construct_uint;

use crate::error::SwapError;

/// global decimal calculate constant variable
pub const GLOBAL_DECIMAL: u64 = 1000000000000000000;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

impl U256 {
    /// Convert u256 to u64
    pub fn to_u64(val: U256) -> Result<u64, SwapError> {
        val.try_into().map_err(|_| SwapError::ConversionFailure)
    }

    /// Convert u256 to u128
    pub fn to_u128(val: U256) -> Result<u128, SwapError> {
        val.try_into().map_err(|_| SwapError::ConversionFailure)
    }

    /// div with ceil
    pub fn checked_ceil_div(self, other: Self) -> Option<Self> {
        if other.is_zero() {
            return None;
        }
        let (quotient, rem) = self.div_mod(other);
        if rem.is_zero() {
            Some(quotient)
        } else {
            quotient.checked_add(1.into())
        }
    }

    /// calculate sqrt
    pub fn sqrt(&self) -> Option<U256> {
        let two: U256 = 2.into();

        let mut z = self.checked_add(U256::one())?.checked_div(two)?;

        let mut y = *self;

        while z < y {
            y = z;
            z = self.checked_div(z)?.checked_add(z)?.checked_div(two)?;
        }

        Some(y)
    }
}

/// FixedU256 struct
#[derive(Clone, Copy, Debug, PartialEq, Ord, PartialOrd, Eq)]
pub struct FixedU256 {
    inner: U256,
    /// 10**precision
    base_point: U256,
}

impl Default for FixedU256 {
    fn default() -> Self {
        Self::new_from_float(0.into(), 18)
    }
}

impl FixedU256 {
    /// Returns a new [`FixedU256`] from an integer not in fixed-point representation.
    pub fn new_from_int(value: U256, precision: u8) -> Option<Self> {
        let base_point = U256::from(10).pow(precision.into());
        Some(Self {
            inner: value.checked_mul(base_point)?,
            base_point,
        })
    }

    /// Returns a new ['FixedU256'] from a value in float
    pub fn new_from_float(value: f64, precision: u8) -> Self {
        let fixed = value * 10f64.powi(precision as i32);
        Self::new_from_fixed(U256::from(fixed.round() as u128), precision)
    }

    /// Returns a new [`FixedU256`] from a value already in a fixed-point representation.
    pub fn new_from_fixed(value: U256, precision: u8) -> Self {
        let base_point = U256::from(10).pow(precision.into());
        Self {
            inner: value,
            base_point,
        }
    }

    /// Return One = 10**18
    pub fn one() -> Self {
        Self::new_from_float(1.into(), 18)
    }

    /// Returns Square Roof of `self`
    ///
    pub fn sqrt(&self) -> Option<Self> {
        let mut x = self.inner.sqrt()?;
        x = x.checked_mul(self.base_point.sqrt()?)?;

        Some(Self {
            inner: x,
            base_point: self.base_point,
        })
    }

    /// Returns 'self - other', rounded up after 'precision' decimal places, use self's precision.
    pub fn checked_sub(&self, other: Self) -> Option<Self> {
        Some(Self {
            inner: self
                .inner
                .checked_mul(GLOBAL_DECIMAL.into())?
                .checked_div(self.base_point)?
                .checked_sub(
                    other
                        .inner
                        .checked_mul(GLOBAL_DECIMAL.into())?
                        .checked_div(other.base_point)?,
                )?
                .checked_mul(self.base_point)?
                .checked_ceil_div(GLOBAL_DECIMAL.into())?,
            base_point: self.base_point,
        })
    }

    /// Returns 'self + other', rounded up after 'precision' decimal places, use self's precision.
    pub fn checked_add(&self, other: Self) -> Option<Self> {
        Some(Self {
            inner: self
                .inner
                .checked_ceil_div(self.base_point)?
                .checked_add(other.inner.checked_ceil_div(other.base_point)?)?
                .checked_mul(self.base_point)?,
            base_point: self.base_point,
        })
    }

    /// Returns `self * other`, rounded up after `precision` decimal places.
    pub fn checked_mul_ceil(&self, other: Self) -> Option<Self> {
        Some(Self {
            inner: self
                .inner
                .checked_mul(other.inner)?
                .checked_ceil_div(other.base_point)?,
            base_point: self.base_point,
        })
    }

    /// Returns `self * other`, rounded down after `precision` decimal places.
    pub fn checked_mul_floor(&self, other: Self) -> Option<Self> {
        Some(Self {
            inner: self
                .inner
                .checked_mul(other.inner)?
                .checked_div(other.base_point)?,
            base_point: self.base_point,
        })
    }

    /// Returns `self / other`, rounded up after `precision` decimal places.
    ///
    /// The output precision will be the same as `self`.
    pub fn checked_div_ceil(&self, other: Self) -> Option<Self> {
        Some(Self {
            inner: self
                .inner
                .checked_mul(other.base_point)?
                .checked_ceil_div(other.inner)?,
            base_point: self.base_point,
        })
    }

    /// Returns `self / other`, rounded down after `precision` decimal places.
    ///
    /// The output precision will be the same as `self`.
    pub fn checked_div_floor(&self, other: Self) -> Option<Self> {
        Some(Self {
            inner: self
                .inner
                .checked_mul(other.base_point)?
                .checked_div(other.inner)?,
            base_point: self.base_point,
        })
    }

    /// Returns the non-fixed point representation, discarding the fractional component.
    pub fn into_u256_floor(self) -> U256 {
        self.inner.checked_div(self.base_point).unwrap_or_default()
    }

    /// Returns the non-fixed point representation, rounding up the fractional component.
    pub fn into_u256_ceil(self) -> U256 {
        self.inner
            .checked_ceil_div(self.base_point)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let a = FixedU256::new_from_float(1.23456, 8);
        let b = FixedU256::new_from_int(42.into(), 0).unwrap();
        let c = FixedU256::new_from_int(4.into(), 0).unwrap();
        assert_eq!(a.checked_mul_ceil(b).unwrap().into_u256_ceil(), 52.into());
        assert_eq!(b.checked_mul_ceil(a).unwrap().into_u256_ceil(), 52.into());
        assert_eq!(c.sqrt().unwrap().into_u256_ceil(), 2.into());
        assert_eq!(b.checked_sub(c).unwrap().into_u256_ceil(), 38.into());
        assert_eq!(b.checked_add(c).unwrap().into_u256_ceil(), 46.into());
    }
}
