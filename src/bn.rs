//! Big number types

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]

use std::{cmp::Ordering, convert::TryInto};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
};
use uint::construct_uint;

use crate::error::SwapError;

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
    pub fn checked_ceil_div(&self, other: Self) -> Result<Self, ProgramError> {
        if other.is_zero() {
            return Err(ProgramError::InvalidArgument);
        }
        let (quotient, rem) = self.div_mod(other);
        if rem.is_zero() {
            Ok(quotient)
        } else {
            Ok(quotient.checked_add(1.into()).unwrap())
        }
    }

    /// div with floor
    pub fn checked_floor_div(&self, other: Self) -> Result<Self, ProgramError> {
        if other.is_zero() {
            return Err(ProgramError::InvalidArgument);
        }
        let (quotient, _rem) = self.div_mod(other);
        Ok(quotient)
    }

    /// mul with ProgramError
    pub fn checked_bn_mul(&self, other: Self) -> Result<Self, ProgramError> {
        match self.checked_mul(other) {
            Some(v) => Ok(v),
            None => Err(ProgramError::InvalidArgument),
        }
    }

    /// add with ProgramError
    pub fn checked_bn_add(&self, other: Self) -> Result<Self, ProgramError> {
        match self.checked_add(other) {
            Some(v) => Ok(v),
            None => Err(ProgramError::InvalidArgument),
        }
    }

    /// sub with ProgramError
    pub fn checked_bn_sub(&self, other: Self) -> Result<Self, ProgramError> {
        match self.checked_sub(other) {
            Some(v) => Ok(v),
            None => Err(ProgramError::InvalidArgument),
        }
    }

    /// calculate sqrt
    pub fn sqrt(&self) -> Result<U256, ProgramError> {
        let two: U256 = 2.into();

        let mut z = self
            .checked_add(U256::one())
            .unwrap()
            .checked_floor_div(two)?;

        let mut y = *self;

        while z < y {
            y = z;
            z = self
                .checked_floor_div(z)?
                .checked_bn_add(z)?
                .checked_floor_div(two)?;
        }

        Ok(y)
    }
}

/// FixedU256 struct
#[derive(Clone, Copy, Debug, PartialEq, Ord, PartialOrd, Eq)]
pub struct FixedU256 {
    /// 10**precision * value
    inner: U256,

    /// 10**precision
    base_point: U256,
}

impl Default for FixedU256 {
    fn default() -> Self {
        Self {
            inner: U256::zero(),
            base_point: U256::zero(),
        }
    }
}

impl FixedU256 {
    /// Getter function for inner
    pub fn inner(&self) -> U256 {
        self.inner
    }

    /// Getter function for base_point
    pub fn base_point(&self) -> U256 {
        self.base_point
    }

    /// Return a new [`FixedU256`] from an integer without fixed-point
    pub fn new(value: U256) -> Self {
        Self {
            inner: value,
            base_point: U256::one(),
        }
    }

    /// Returns a new [`FixedU256`] from an integer not in fixed-point representation.
    pub fn new_from_int(value: U256, precision: u8) -> Result<Self, ProgramError> {
        let base_point = U256::from(10).pow(precision.into());
        match value.checked_mul(base_point) {
            Some(v) => Ok(Self {
                inner: v,
                base_point,
            }),
            None => Err(ProgramError::InvalidArgument),
        }
    }

    /// Returns a new ['FixedU256'] from a value in float
    // pub fn new_from_float(value: f64, precision: u8) -> Self {
    //     let fixed = value * 10f64.powi(precision as i32);
    //     Self::new_from_fixed(U256::from(fixed.round() as u128), precision)
    // }

    /// Returns a new [`FixedU256`] from a value already in a fixed-point representation.
    pub fn new_from_fixed(value: U256, precision: u8) -> Self {
        let base_point = U256::from(10).pow(precision.into());
        Self {
            inner: value,
            base_point,
        }
    }

    /// Return zero = 0, 10**18
    pub fn zero() -> Self {
        Self::new_from_int(U256::zero(), 18).unwrap()
    }

    /// Return One = 10**18
    pub fn one() -> Self {
        Self::new_from_int(U256::one(), 18).unwrap()
    }

    /// Return One2 = 10**36
    pub fn one2() -> Self {
        Self::new_from_int(U256::from(10).pow(18.into()), 18).unwrap()
    }

    /// Return a new ['FixedU256'] with new base point
    pub fn take_and_scale(&self, new_base_point: U256) -> Result<FixedU256, ProgramError> {
        if self.inner.is_zero() {
            return Ok(Self {
                inner: U256::zero(),
                base_point: new_base_point,
            });
        }

        match new_base_point.cmp(&self.base_point) {
            Ordering::Greater => {
                let value = self
                    .inner
                    .checked_bn_mul(new_base_point.checked_floor_div(self.base_point)?)?;
                Ok(Self {
                    inner: value,
                    base_point: new_base_point,
                })
            }
            Ordering::Less => {
                let value = self
                    .inner
                    .checked_ceil_div(self.base_point.checked_floor_div(new_base_point)?)?;

                Ok(Self {
                    inner: value,
                    base_point: new_base_point,
                })
            }
            Ordering::Equal => Ok(*self),
        }
    }

    /// Returns Square Roof of `self`
    pub fn sqrt(&self) -> Result<Self, ProgramError> {
        let mut x = self.inner.sqrt()?;
        x = x.checked_bn_mul(self.base_point.sqrt()?)?;

        Ok(Self {
            inner: x,
            base_point: self.base_point,
        })
    }

    /// Returns 'self - other', rounded up after 'precision' decimal places, use self's precision.
    pub fn checked_sub(&self, other: Self) -> Result<Self, ProgramError> {
        match self.base_point.cmp(&other.base_point) {
            Ordering::Equal => Ok(Self {
                inner: self.inner.checked_bn_sub(other.inner)?,
                base_point: self.base_point,
            }),
            Ordering::Less => {
                let new_other = other.take_and_scale(self.base_point)?;
                Ok(Self {
                    inner: self.inner.checked_bn_sub(new_other.inner)?,
                    base_point: self.base_point,
                })
            }
            Ordering::Greater => {
                let new_other = other.take_and_scale(self.base_point)?;
                Ok(Self {
                    inner: self.inner.checked_bn_sub(new_other.inner)?,
                    base_point: self.base_point,
                })
            }
        }
    }

    /// Returns 'self + other', rounded up after 'precision' decimal places, use self's precision.
    pub fn checked_add(&self, other: Self) -> Result<Self, ProgramError> {
        match self.base_point.cmp(&other.base_point) {
            Ordering::Equal => Ok(Self {
                inner: self.inner.checked_bn_add(other.inner)?,
                base_point: self.base_point,
            }),
            Ordering::Less => {
                let new_other = other.take_and_scale(self.base_point)?;
                Ok(Self {
                    inner: self.inner.checked_bn_add(new_other.inner)?,
                    base_point: self.base_point,
                })
            }
            Ordering::Greater => {
                let new_other = other.take_and_scale(self.base_point)?;
                Ok(Self {
                    inner: self.inner.checked_bn_add(new_other.inner)?,
                    base_point: self.base_point,
                })
            }
        }
    }

    /// Returns `self * other`, rounded up after `precision` decimal places.
    pub fn checked_mul_ceil(&self, other: Self) -> Result<Self, ProgramError> {
        let value = self
            .inner
            .checked_bn_mul(other.inner)?
            .checked_ceil_div(other.base_point)?;
        Ok(Self {
            inner: value,
            base_point: self.base_point,
        })
    }

    /// Returns `self * other`, rounded down after `precision` decimal places.
    pub fn checked_mul_floor(&self, other: Self) -> Result<Self, ProgramError> {
        let value = self
            .inner
            .checked_bn_mul(other.inner)?
            .checked_floor_div(other.base_point)?;
        Ok(Self {
            inner: value,
            base_point: self.base_point,
        })
    }

    /// Returns `self / other`, rounded up after `precision` decimal places.
    ///
    /// The output precision will be the same as `self`.
    pub fn checked_div_ceil(&self, other: Self) -> Result<Self, ProgramError> {
        let new_other = other.take_and_scale(self.base_point)?;
        let value;
        if self.inner >= new_other.inner {
            value = self
                .inner
                .checked_ceil_div(other.inner)?
                .checked_bn_mul(other.base_point)?;
        } else {
            value = self
                .inner
                .checked_bn_mul(other.base_point)?
                .checked_ceil_div(other.inner)?;
        }

        Ok(Self {
            inner: value,
            base_point: self.base_point,
        })
    }

    /// Returns `self / other`, rounded down after `precision` decimal places.
    ///
    /// The output precision will be the same as `self`.
    pub fn checked_div_floor(&self, other: Self) -> Result<Self, ProgramError> {
        let new_other = other.take_and_scale(self.base_point)?;
        let value;
        if self.inner >= new_other.inner {
            value = self
                .inner
                .checked_floor_div(other.inner)?
                .checked_bn_mul(other.base_point)?;
        } else {
            value = self
                .inner
                .checked_bn_mul(other.base_point)?
                .checked_floor_div(other.inner)?;
        }

        Ok(Self {
            inner: value,
            base_point: self.base_point,
        })
    }

    /// calculate 1/target - floor
    pub fn reciprocal_floor(target: FixedU256) -> Result<Self, ProgramError> {
        FixedU256::one().checked_div_floor(target)
    }

    /// calculate 1/target - ceil
    pub fn reciprocal_ceil(target: FixedU256) -> Result<Self, ProgramError> {
        FixedU256::one().checked_div_ceil(target)
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

impl Sealed for FixedU256 {}
impl Pack for FixedU256 {
    const LEN: usize = 64;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 64];
        #[allow(clippy::ptr_offset_with_cast)]
        let (inner, base_point) = array_refs![input, 32, 32];
        Ok(Self {
            inner: U256::from_little_endian(inner),
            base_point: U256::from_little_endian(base_point),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 64];
        let (inner, base_point) = mut_array_refs![output, 32, 32];
        self.inner.to_little_endian(inner);
        self.base_point.to_little_endian(base_point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let a = FixedU256::new_from_int(2.into(), 0).unwrap();
        let b = FixedU256::new_from_int(42.into(), 0).unwrap();
        let c = FixedU256::new_from_int(4.into(), 0).unwrap();
        let d = FixedU256::new_from_int(2.into(), 18)
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();
        assert_eq!(
            FixedU256::reciprocal_floor(FixedU256::new(5.into())).unwrap(),
            d
        );
        assert_eq!(
            FixedU256::reciprocal_ceil(FixedU256::new(5.into())).unwrap(),
            d
        );
        assert_eq!(b.checked_mul_ceil(a).unwrap().into_u256_ceil(), 84.into());
        assert_eq!(c.sqrt().unwrap().into_u256_ceil(), 2.into());
        assert_eq!(b.checked_sub(c).unwrap().into_u256_ceil(), 38.into());
        assert_eq!(b.checked_add(c).unwrap().into_u256_ceil(), 46.into());
        assert_eq!(
            FixedU256::one().checked_add(c).unwrap().into_u256_ceil(),
            5.into()
        );
        assert_eq!(FixedU256::new(4.into()).into_u256_ceil(), 4.into());
        assert_eq!(
            b.take_and_scale(100.into()).unwrap().into_u256_ceil(),
            42.into()
        );
        assert_eq!(
            b.checked_sub(FixedU256::new_from_int(1.into(), 2).unwrap())
                .unwrap()
                .into_u256_ceil(),
            41.into()
        );
    }
}
