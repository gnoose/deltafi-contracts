//! Math for preserving precision of token amounts which are limited
//! by the SPL Token program to be at most u64::MAX.
//!
//! Decimals are internally scaled by a WAD (10^18) to preserve
//! precision up to 18 decimal places. Decimals are sized to support
//! both serialization and precise math for the full range of
//! unsigned 64-bit integers. The underlying representation is a
//! u192 rather than u256 to reduce compute cost while losing
//! support for arithmetic operations at the high end of u64 range.

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]

use super::*;
use crate::error::SwapError;
use solana_program::program_error::ProgramError;
use std::{convert::TryFrom, fmt};

use uint::construct_uint;

construct_uint! {
    pub struct U192(3);
}

/// Large decimal values, precise to 18 digits
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct Decimal(pub U192);

impl Decimal {
    /// One
    pub fn one() -> Self {
        Self(Self::wad())
    }

    /// Zero
    pub fn zero() -> Self {
        Self(U192::zero())
    }

    /// Check if zero
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    // OPTIMIZE: use const slice when fixed in BPF toolchain
    fn wad() -> U192 {
        U192::from(WAD)
    }

    // OPTIMIZE: use const slice when fixed in BPF toolchain
    fn half_wad() -> U192 {
        U192::from(HALF_WAD)
    }

    /// Create scaled decimal from percent value
    pub fn from_percent(percent: u8) -> Self {
        Self(U192::from(percent as u64 * PERCENT_SCALER))
    }

    /// Return raw scaled value if it fits within u128
    #[allow(clippy::wrong_self_convention)]
    pub fn to_scaled_val(&self) -> Result<u128, ProgramError> {
        Ok(u128::try_from(self.0).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Create decimal from scaled value
    pub fn from_scaled_val(scaled_val: u128) -> Self {
        Self(U192::from(scaled_val))
    }

    /// Round scaled decimal to u128
    pub fn try_round_u128(&self) -> Result<u128, ProgramError> {
        let rounded_val = Self::half_wad()
            .checked_add(self.0)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(Self::wad())
            .ok_or(SwapError::CalculationFailure)?;
        Ok(u128::try_from(rounded_val).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Ceiling scaled decimal to u128
    pub fn try_ceil_u128(&self) -> Result<u128, ProgramError> {
        let ceil_val = Self::wad()
            .checked_sub(U192::from(1u64))
            .ok_or(SwapError::CalculationFailure)?
            .checked_add(self.0)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(Self::wad())
            .ok_or(SwapError::CalculationFailure)?;
        Ok(u128::try_from(ceil_val).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Floor scaled decimal to u128
    pub fn try_floor_u128(&self) -> Result<u128, ProgramError> {
        let ceil_val = self
            .0
            .checked_div(Self::wad())
            .ok_or(SwapError::CalculationFailure)?;
        Ok(u128::try_from(ceil_val).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Round scaled decimal to u64
    pub fn try_round_u64(&self) -> Result<u64, ProgramError> {
        let rounded_val = Self::half_wad()
            .checked_add(self.0)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(Self::wad())
            .ok_or(SwapError::CalculationFailure)?;
        Ok(u64::try_from(rounded_val).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Ceiling scaled decimal to u64
    pub fn try_ceil_u64(&self) -> Result<u64, ProgramError> {
        let ceil_val = Self::wad()
            .checked_sub(U192::from(1u64))
            .ok_or(SwapError::CalculationFailure)?
            .checked_add(self.0)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(Self::wad())
            .ok_or(SwapError::CalculationFailure)?;
        Ok(u64::try_from(ceil_val).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Floor scaled decimal to u64
    pub fn try_floor_u64(&self) -> Result<u64, ProgramError> {
        let ceil_val = self
            .0
            .checked_div(Self::wad())
            .ok_or(SwapError::CalculationFailure)?;
        Ok(u64::try_from(ceil_val).map_err(|_| SwapError::CalculationFailure)?)
    }

    /// Square root decimal
    pub fn sqrt(&self) -> Result<Self, ProgramError> {
        Ok(Self::from(
            sqrt(self.try_round_u128()?).ok_or(SwapError::CalculationFailure)?,
        ))
    }

    /// Reciprocal decimal
    pub fn reciprocal(&self) -> Result<Self, ProgramError> {
        Ok(Self(
            Self::wad()
                .checked_pow(U192::from(2u64))
                .ok_or(SwapError::CalculationFailure)?
                .checked_div(self.0)
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut scaled_val = self.0.to_string();
        if scaled_val.len() <= SCALE {
            scaled_val.insert_str(0, &vec!["0"; SCALE - scaled_val.len()].join(""));
            scaled_val.insert_str(0, "0.");
        } else {
            scaled_val.insert(scaled_val.len() - SCALE, '.');
        }
        f.write_str(&scaled_val)
    }
}

impl Default for Decimal {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<u64> for Decimal {
    fn from(val: u64) -> Self {
        Self(Self::wad() * U192::from(val))
    }
}

impl From<u128> for Decimal {
    fn from(val: u128) -> Self {
        Self(Self::wad() * U192::from(val))
    }
}

impl From<Rate> for Decimal {
    fn from(val: Rate) -> Self {
        Self(U192::from(val.to_scaled_val()))
    }
}

impl TryAdd for Decimal {
    fn try_add(self, rhs: Self) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_add(rhs.0)
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TrySub for Decimal {
    fn try_sub(self, rhs: Self) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_sub(rhs.0)
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryDiv<u64> for Decimal {
    fn try_div(self, rhs: u64) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_div(U192::from(rhs))
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryDiv<Rate> for Decimal {
    fn try_div(self, rhs: Rate) -> Result<Self, ProgramError> {
        self.try_div(Self::from(rhs))
    }
}

impl TryDiv<Decimal> for Decimal {
    fn try_div(self, rhs: Self) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_mul(Self::wad())
                .ok_or(SwapError::CalculationFailure)?
                .checked_div(rhs.0)
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryMul<u64> for Decimal {
    fn try_mul(self, rhs: u64) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_mul(U192::from(rhs))
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryMul<Rate> for Decimal {
    fn try_mul(self, rhs: Rate) -> Result<Self, ProgramError> {
        self.try_mul(Self::from(rhs))
    }
}

impl TryMul<Decimal> for Decimal {
    fn try_mul(self, rhs: Self) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_mul(rhs.0)
                .ok_or(SwapError::CalculationFailure)?
                .checked_div(Self::wad())
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_decimal() {
        assert_eq!(Decimal::from(0u64), Decimal::zero());
        assert_eq!(Decimal::from(1u64), Decimal::one());
        assert_eq!(Decimal::from(0u64), Decimal::zero());
        assert_eq!(Decimal::from(1u64), Decimal::one());
        assert_eq!(Decimal::from(Rate::zero()), Decimal::zero());
        assert_eq!(Decimal::from(Rate::one()), Decimal::one());

        assert_eq!(Decimal::from_percent(0u8), Decimal::zero());
        assert_eq!(Decimal::from_percent(100u8), Decimal::one());

        assert_eq!(Decimal::from_scaled_val(0u128).to_scaled_val().unwrap(), 0);
        assert_eq!(
            Decimal::from_scaled_val(100u128).to_scaled_val().unwrap(),
            100
        );
        assert_eq!(
            Decimal::from_scaled_val(u128::MAX).to_scaled_val().unwrap(),
            u128::MAX
        );

        assert_eq!(Decimal::wad(), U192::from(WAD));
        assert_eq!(Decimal::half_wad(), U192::from(HALF_WAD));
        assert_eq!(Decimal::zero().to_scaled_val().unwrap(), 0);
        assert!(Decimal::zero().is_zero());
        assert!(!Decimal::one().is_zero());
        assert_eq!(Decimal::default(), Decimal::zero());

        assert_eq!(Decimal::one().try_round_u64().unwrap(), 1u64);
        assert_eq!(Decimal::zero().try_round_u64().unwrap(), 0u64);
        assert_eq!(Decimal::from_scaled_val(1).try_round_u64().unwrap(), 0u64);
        assert_eq!(Decimal::from_scaled_val(100).try_round_u64().unwrap(), 0u64);
        assert_eq!(
            Decimal::from_scaled_val(HALF_WAD as u128)
                .try_round_u64()
                .unwrap(),
            1u64
        );
        assert_eq!(
            Decimal::from_scaled_val(WAD as u128)
                .try_round_u64()
                .unwrap(),
            1u64
        );
        assert_eq!(
            Decimal::from_scaled_val(WAD as u128 * 2)
                .try_round_u64()
                .unwrap(),
            2u64
        );

        assert_eq!(Decimal::one().try_ceil_u64().unwrap(), 1u64);
        assert_eq!(Decimal::zero().try_ceil_u64().unwrap(), 0u64);
        assert_eq!(Decimal::from_scaled_val(1).try_ceil_u64().unwrap(), 1u64);
        assert_eq!(Decimal::from_scaled_val(100).try_ceil_u64().unwrap(), 1u64);
        assert_eq!(
            Decimal::from_scaled_val(HALF_WAD as u128)
                .try_ceil_u64()
                .unwrap(),
            1u64
        );
        assert_eq!(
            Decimal::from_scaled_val(WAD as u128)
                .try_ceil_u64()
                .unwrap(),
            1u64
        );
        assert_eq!(
            Decimal::from_scaled_val(WAD as u128 * 2)
                .try_ceil_u64()
                .unwrap(),
            2u64
        );

        assert_eq!(Decimal::one().try_floor_u64().unwrap(), 1u64);
        assert_eq!(Decimal::zero().try_floor_u64().unwrap(), 0u64);
        assert_eq!(Decimal::from_scaled_val(1).try_floor_u64().unwrap(), 0u64);
        assert_eq!(Decimal::from_scaled_val(100).try_floor_u64().unwrap(), 0u64);
        assert_eq!(
            Decimal::from_scaled_val(HALF_WAD as u128)
                .try_floor_u64()
                .unwrap(),
            0u64
        );
        assert_eq!(
            Decimal::from_scaled_val(WAD as u128)
                .try_floor_u64()
                .unwrap(),
            1u64
        );
        assert_eq!(
            Decimal::from_scaled_val(WAD as u128 * 2)
                .try_floor_u64()
                .unwrap(),
            2u64
        );

        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_mul(Rate::from_scaled_val(2u128))
                .unwrap(),
            Decimal::zero()
        );
        assert_eq!(
            Decimal::from_scaled_val(0)
                .try_mul(Rate::from_scaled_val(2u128))
                .unwrap(),
            Decimal::from_scaled_val(0)
        );
        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_mul(Rate::from_scaled_val(2u128))
                .unwrap(),
            Decimal::from_scaled_val(0)
        );
        assert_eq!(
            Decimal::from_scaled_val(2).try_mul(Decimal::one()).unwrap(),
            Decimal::from_scaled_val(2)
        );
        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_mul(Decimal::from_scaled_val(WAD as u128 * 2))
                .unwrap(),
            Decimal::from_scaled_val(4)
        );

        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_div(Rate::from_scaled_val(2u128))
                .unwrap(),
            Decimal::one()
        );
        assert_eq!(
            Decimal::from_scaled_val(0)
                .try_div(Rate::from_scaled_val(2u128))
                .unwrap(),
            Decimal::from_scaled_val(0)
        );
        assert_eq!(
            Decimal::from_scaled_val(2).try_div(Decimal::one()).unwrap(),
            Decimal::from_scaled_val(2)
        );
        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_div(Decimal::from_scaled_val(WAD as u128 * 2))
                .unwrap(),
            Decimal::from_scaled_val(1)
        );
        assert!(Decimal::from_scaled_val(2)
            .try_div(Decimal::from_scaled_val(0))
            .is_err());

        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_add(Decimal::from_scaled_val(2))
                .unwrap(),
            Decimal::from_scaled_val(4)
        );
        assert_eq!(
            Decimal::from_scaled_val(0)
                .try_add(Decimal::from_scaled_val(2))
                .unwrap(),
            Decimal::from_scaled_val(2)
        );

        assert_eq!(
            Decimal::from_scaled_val(2)
                .try_sub(Decimal::from_scaled_val(2))
                .unwrap(),
            Decimal::from_scaled_val(0)
        );
        assert_eq!(
            Decimal::from_scaled_val(u128::MAX)
                .try_sub(Decimal::from_scaled_val(u128::MAX))
                .unwrap(),
            Decimal::from_scaled_val(0)
        );
        assert!(Decimal::from_scaled_val(0)
            .try_sub(Decimal::from_scaled_val(2))
            .is_err());

        assert_eq!(&format!("{}", Decimal::one()), "1.000000000");
        assert_eq!(&format!("{}", Decimal::from_scaled_val(2)), "0.000000002");

        assert_eq!(U192::exp10(SCALE), Decimal::wad());
    }
}
