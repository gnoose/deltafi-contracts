//! Big number types

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]

use std::{cmp::Ordering, convert::TryInto};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use num_traits::Zero;
use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
};
use uint::construct_uint;

use crate::{
    error::SwapError,
    math2::{checked_bn_add, checked_bn_mul, checked_bn_sub, checked_ceil_div, sqrt},
    utils::DEFAULT_TOKEN_DECIMALS,
};

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

/// FixedU64 struct
#[derive(Clone, Copy, Debug, PartialEq, Ord, PartialOrd, Eq, Default)]
pub struct FixedU64 {
    /// 10**precision * value
    inner: u64,

    /// 10**precision
    precision: u8,
}

impl FixedU64 {
    /// Getter function for inner
    pub fn inner(&self) -> u64 {
        self.inner
    }

    /// Getter function for precision
    pub fn precision(&self) -> u8 {
        self.precision
    }

    /// Getter function for base_point
    pub fn base_point(&self) -> u64 {
        10u64.pow(self.precision as u32)
    }

    /// Return a new [`FixedU64`] from an integer without fixed-point
    pub fn new(value: u64) -> Self {
        Self {
            inner: value,
            precision: 0,
        }
    }

    /// Returns a new [`FixedU64`] from an integer not in fixed-point representation.
    pub fn new_from_int(value: u64, precision: u8) -> Result<Self, ProgramError> {
        let base_point = 10u64.pow(precision as u32);
        match value.checked_mul(base_point) {
            Some(v) => Ok(Self {
                inner: v,
                precision,
            }),
            None => Err(ProgramError::InvalidArgument),
        }
    }

    /// Returns a new [`FixedU64`] from a value already in a fixed-point representation.
    pub fn new_from_u64(value: u64) -> Result<Self, ProgramError> {
        let base_point = 10u64.pow(DEFAULT_TOKEN_DECIMALS as u32);
        match value.checked_mul(base_point) {
            Some(v) => Ok(Self {
                inner: v,
                precision: DEFAULT_TOKEN_DECIMALS,
            }),
            None => Err(ProgramError::InvalidArgument),
        }
    }

    /// Returns a new [`FixedU64`] from a value already in a fixed-point representation.
    pub fn new_from_fixed_u64(value: u64) -> Result<Self, ProgramError> {
        Ok(Self {
            inner: value,
            precision: DEFAULT_TOKEN_DECIMALS,
        })
    }

    /// Return zero = 0, 10**6
    pub fn zero() -> Self {
        Self::new_from_int(0, DEFAULT_TOKEN_DECIMALS).unwrap()
    }

    /// Return One = 10**6
    pub fn one() -> Self {
        Self::new_from_int(1, DEFAULT_TOKEN_DECIMALS).unwrap()
    }

    /// Return a new ['FixedU64'] with new base point
    pub fn take_and_scale(&self, new_precision: u8) -> Result<FixedU64, ProgramError> {
        if self.inner.is_zero() {
            return Ok(Self {
                inner: 0,
                precision: new_precision,
            });
        }

        match new_precision.cmp(&self.precision) {
            Ordering::Greater => {
                let value = checked_bn_mul(
                    self.inner,
                    10u64.pow(checked_bn_sub(new_precision as u64, self.precision as u64)? as u32),
                )?;
                Ok(Self {
                    inner: value,
                    precision: new_precision,
                })
            }
            Ordering::Less => {
                let value = checked_ceil_div(
                    self.inner,
                    10u64.pow(checked_bn_sub(self.precision as u64, new_precision as u64)? as u32),
                )?;
                Ok(Self {
                    inner: value,
                    precision: new_precision,
                })
            }
            Ordering::Equal => Ok(*self),
        }
    }

    /// Returns Square Roof of `self`
    pub fn sqrt(&self) -> Result<Self, ProgramError> {
        let mut x = sqrt(self.inner)?;
        x = checked_bn_mul(x, sqrt(self.base_point())?)?;

        Ok(Self {
            inner: x,
            precision: self.precision,
        })
    }

    /// Returns 'self - other', rounded up after 'precision' decimal places, use self's precision.
    pub fn checked_sub(&self, other: Self) -> Result<Self, ProgramError> {
        match self.precision.cmp(&other.precision) {
            Ordering::Equal => Ok(Self {
                inner: checked_bn_sub(self.inner, other.inner)?,
                precision: self.precision,
            }),
            Ordering::Less => {
                let new_other = other.take_and_scale(self.precision)?;
                Ok(Self {
                    inner: checked_bn_sub(self.inner, new_other.inner)?,
                    precision: self.precision,
                })
            }
            Ordering::Greater => {
                let new_other = other.take_and_scale(self.precision)?;
                Ok(Self {
                    inner: checked_bn_sub(self.inner, new_other.inner)?,
                    precision: self.precision,
                })
            }
        }
    }

    /// Returns 'self + other', rounded up after 'precision' decimal places, use self's precision.
    pub fn checked_add(&self, other: Self) -> Result<Self, ProgramError> {
        match self.precision.cmp(&other.precision) {
            Ordering::Equal => Ok(Self {
                inner: checked_bn_add(self.inner, other.inner)?,
                precision: self.precision,
            }),
            Ordering::Less => {
                let new_other = other.take_and_scale(self.precision)?;
                Ok(Self {
                    inner: checked_bn_add(self.inner, new_other.inner)?,
                    precision: self.precision,
                })
            }
            Ordering::Greater => {
                let new_other = other.take_and_scale(self.precision)?;
                Ok(Self {
                    inner: checked_bn_add(self.inner, new_other.inner)?,
                    precision: self.precision,
                })
            }
        }
    }

    /// Returns `self * other`, rounded up after `precision` decimal places.
    pub fn checked_mul_ceil(&self, other: Self) -> Result<Self, ProgramError> {
        let v1 = U256::from(self.inner);
        let v2 = U256::from(other.inner);
        let base_point = U256::from(other.base_point());
        let value = v1.checked_bn_mul(v2)?.checked_ceil_div(base_point)?;
        Ok(Self {
            inner: U256::to_u64(value)?,
            precision: self.precision,
        })
    }

    /// Returns `self * other`, rounded down after `precision` decimal places.
    pub fn checked_mul_floor(&self, other: Self) -> Result<Self, ProgramError> {
        let v1 = U256::from(self.inner);
        let v2 = U256::from(other.inner);
        let base_point = U256::from(other.base_point());
        let value = v1.checked_bn_mul(v2)?.checked_floor_div(base_point)?;
        Ok(Self {
            inner: U256::to_u64(value)?,
            precision: self.precision,
        })
    }

    /// Returns `self / other`, rounded up after `precision` decimal places.
    ///
    /// The output precision will be the same as `self`.
    pub fn checked_div_ceil(&self, other: Self) -> Result<Self, ProgramError> {
        let v1 = U256::from(self.inner);
        let v2 = U256::from(other.inner);
        let base_point = U256::from(other.base_point());
        let value = v1.checked_bn_mul(base_point)?.checked_ceil_div(v2)?;

        Ok(Self {
            inner: U256::to_u64(value)?,
            precision: self.precision,
        })
    }

    /// Returns `self / other`, rounded down after `precision` decimal places.
    ///
    /// The output precision will be the same as `self`.
    pub fn checked_div_floor(&self, other: Self) -> Result<Self, ProgramError> {
        let v1 = U256::from(self.inner);
        let v2 = U256::from(other.inner);
        let base_point = U256::from(other.base_point());
        let value = v1.checked_bn_mul(base_point)?.checked_floor_div(v2)?;

        Ok(Self {
            inner: U256::to_u64(value)?,
            precision: self.precision,
        })
    }

    /// calculate 1/target - floor
    pub fn reciprocal_floor(target: FixedU64) -> Result<Self, ProgramError> {
        FixedU64::one().checked_div_floor(target)
    }

    /// calculate 1/target - ceil
    pub fn reciprocal_ceil(target: FixedU64) -> Result<Self, ProgramError> {
        FixedU64::one().checked_div_ceil(target)
    }

    /// Returns the non-fixed point representation, discarding the fractional component.
    pub fn into_real_u64_floor(self) -> u64 {
        self.inner
            .checked_div(self.base_point())
            .unwrap_or_default()
    }

    /// Returns the non-fixed point representation, rounding up the fractional component - u64.
    pub fn into_real_u64_ceil(self) -> u64 {
        checked_ceil_div(self.inner, self.base_point()).unwrap()
    }
}

impl Sealed for FixedU64 {}
impl Pack for FixedU64 {
    const LEN: usize = 9;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 9];
        #[allow(clippy::ptr_offset_with_cast)]
        let (inner, precision) = array_refs![input, 8, 1];
        Ok(Self {
            inner: u64::from_le_bytes(*inner),
            precision: u8::from_le_bytes(*precision),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 9];
        let (inner, precision) = mut_array_refs![output, 8, 1];
        *inner = self.inner.to_le_bytes();
        *precision = self.precision.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let a = FixedU64::new(2);
        let b = FixedU64::new(42);
        let c = FixedU64::new(4);
        let d = FixedU64::new_from_int(2, DEFAULT_TOKEN_DECIMALS)
            .unwrap()
            .checked_div_floor(FixedU64::new(10))
            .unwrap();
        assert_eq!(FixedU64::reciprocal_floor(FixedU64::new(5)).unwrap(), d);
        assert_eq!(FixedU64::reciprocal_ceil(FixedU64::new(5)).unwrap(), d);
        assert_eq!(b.checked_mul_ceil(a).unwrap().into_real_u64_ceil(), 84);
        assert_eq!(c.sqrt().unwrap().into_real_u64_ceil(), 2);
        assert_eq!(b.checked_sub(c).unwrap().into_real_u64_ceil(), 38);
        assert_eq!(b.checked_add(c).unwrap().into_real_u64_ceil(), 46);
        assert_eq!(
            FixedU64::one().checked_add(c).unwrap().into_real_u64_ceil(),
            5
        );
        assert_eq!(FixedU64::new(4).into_real_u64_ceil(), 4);
        assert_eq!(b.take_and_scale(2).unwrap().into_real_u64_ceil(), 42);
        assert_eq!(
            b.checked_sub(FixedU64::new_from_int(1, 2).unwrap())
                .unwrap()
                .into_real_u64_ceil(),
            41
        );
    }
}
