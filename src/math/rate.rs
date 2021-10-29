//! Math for preserving precision of ratios and percentages.
//!
//! Usages and their ranges include:
//!   - Collateral exchange ratio <= 5.0
//!   - Loan to value ratio <= 0.9
//!   - Max borrow rate <= 2.56
//!   - Percentages <= 1.0
//!
//! Rates are internally scaled by a WAD (10^18) to preserve
//! precision up to 18 decimal places. Rates are sized to support
//! both serialization and precise math for the full range of
//! unsigned 8-bit integers. The underlying representation is a
//! u128 rather than u192 to reduce compute cost while losing
//! support for arithmetic operations at the high end of u8 range.

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::reversed_empty_ranges)]
#![allow(clippy::manual_range_contains)]

use super::*;
use crate::error::SwapError;
use solana_program::program_error::ProgramError;
use std::{convert::TryFrom, fmt};

use uint::construct_uint;

construct_uint! {
    /// 128-bit unsigned integer
    pub struct U128(2);
}

/// Small decimal values, precise to 18 digits
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Rate(pub U128);

impl Rate {
    /// One
    pub fn one() -> Self {
        Self(Self::wad())
    }

    /// Zero
    pub fn zero() -> Self {
        Self(U128::from(0))
    }
    /// OPTIMIZE: use const slice when fixed in BPF toolchain
    fn wad() -> U128 {
        U128::from(WAD)
    }

    /// OPTIMIZE: use const slice when fixed in BPF toolchain
    fn half_wad() -> U128 {
        U128::from(HALF_WAD)
    }

    /// Create scaled decimal from percent value
    pub fn from_percent(percent: u8) -> Self {
        Self(U128::from(percent as u64 * PERCENT_SCALER))
    }

    /// Return raw scaled value
    #[allow(clippy::wrong_self_convention)]
    pub fn to_scaled_val(&self) -> u128 {
        self.0.as_u128()
    }

    /// Create decimal from scaled value
    pub fn from_scaled_val(scaled_val: u128) -> Self {
        Self(U128::from(scaled_val))
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

    /// Calculates base^exp
    pub fn try_pow(&self, mut exp: u64) -> Result<Rate, ProgramError> {
        let mut base = *self;
        let mut ret = if exp % 2 != 0 {
            base
        } else {
            Rate(Self::wad())
        };

        while exp > 0 {
            exp /= 2;
            base = base.try_mul(base)?;

            if exp % 2 != 0 {
                ret = ret.try_mul(base)?;
            }
        }

        Ok(ret)
    }
}

impl fmt::Display for Rate {
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

impl TryFrom<Decimal> for Rate {
    type Error = ProgramError;
    fn try_from(decimal: Decimal) -> Result<Self, Self::Error> {
        Ok(Self(U128::from(decimal.to_scaled_val()?)))
    }
}

impl TryAdd for Rate {
    fn try_add(self, rhs: Self) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_add(rhs.0)
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TrySub for Rate {
    fn try_sub(self, rhs: Self) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_sub(rhs.0)
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryDiv<u64> for Rate {
    fn try_div(self, rhs: u64) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_div(U128::from(rhs))
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryDiv<Rate> for Rate {
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

impl TryMul<u64> for Rate {
    fn try_mul(self, rhs: u64) -> Result<Self, ProgramError> {
        Ok(Self(
            self.0
                .checked_mul(U128::from(rhs))
                .ok_or(SwapError::CalculationFailure)?,
        ))
    }
}

impl TryMul<Rate> for Rate {
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
    fn test_rate() {
        assert_eq!(Rate::wad(), U128::from(WAD));
        assert_eq!(Rate::one().to_scaled_val(), WAD as u128);
        assert_eq!(Rate::half_wad(), U128::from(HALF_WAD));
        assert_eq!(Rate::zero().to_scaled_val(), 0);

        assert_eq!(Rate::from_percent(0u8), Rate::zero());
        assert_eq!(Rate::from_percent(100u8), Rate::one());

        assert_eq!(Rate::from_scaled_val(0u128).to_scaled_val(), 0);
        assert_eq!(Rate::from_scaled_val(100u128).to_scaled_val(), 100);
        assert_eq!(Rate::from_scaled_val(u128::MAX).to_scaled_val(), u128::MAX);

        assert_eq!(Rate::one().try_round_u64().unwrap(), 1u64);
        assert_eq!(Rate::zero().try_round_u64().unwrap(), 0u64);
        assert_eq!(Rate::from_scaled_val(1).try_round_u64().unwrap(), 0u64);
        assert_eq!(Rate::from_scaled_val(100).try_round_u64().unwrap(), 0u64);
        assert_eq!(
            Rate::from_scaled_val(HALF_WAD as u128)
                .try_round_u64()
                .unwrap(),
            1u64
        );
        assert_eq!(
            Rate::from_scaled_val(WAD as u128).try_round_u64().unwrap(),
            1u64
        );
        assert_eq!(
            Rate::from_scaled_val(WAD as u128 * 2)
                .try_round_u64()
                .unwrap(),
            2u64
        );

        assert_eq!(
            Rate::from_scaled_val(2).try_mul(2u64).unwrap(),
            Rate::from_scaled_val(4)
        );
        assert_eq!(
            Rate::from_scaled_val(0).try_mul(2u64).unwrap(),
            Rate::from_scaled_val(0)
        );
        assert_eq!(
            Rate::from_scaled_val(2).try_mul(0u64).unwrap(),
            Rate::from_scaled_val(0)
        );
        assert_eq!(
            Rate::from_scaled_val(2).try_mul(Rate::one()).unwrap(),
            Rate::from_scaled_val(2)
        );
        assert_eq!(
            Rate::from_scaled_val(2)
                .try_mul(Rate::from_scaled_val(WAD as u128 * 2))
                .unwrap(),
            Rate::from_scaled_val(4)
        );

        assert_eq!(
            Rate::from_scaled_val(2).try_div(2u64).unwrap(),
            Rate::from_scaled_val(1)
        );
        assert_eq!(
            Rate::from_scaled_val(0).try_div(2u64).unwrap(),
            Rate::from_scaled_val(0)
        );
        assert_eq!(
            Rate::from_scaled_val(2).try_div(Rate::one()).unwrap(),
            Rate::from_scaled_val(2)
        );
        assert_eq!(
            Rate::from_scaled_val(2)
                .try_div(Rate::from_scaled_val(WAD as u128 * 2))
                .unwrap(),
            Rate::from_scaled_val(1)
        );
        assert!(Rate::from_scaled_val(2).try_div(0u64).is_err());

        assert_eq!(
            Rate::from_scaled_val(2)
                .try_add(Rate::from_scaled_val(2))
                .unwrap(),
            Rate::from_scaled_val(4)
        );
        assert_eq!(
            Rate::from_scaled_val(0)
                .try_add(Rate::from_scaled_val(2))
                .unwrap(),
            Rate::from_scaled_val(2)
        );
        assert!(Rate::from_scaled_val(u128::MAX)
            .try_add(Rate::from_scaled_val(u128::MAX))
            .is_err());

        assert_eq!(
            Rate::from_scaled_val(2)
                .try_sub(Rate::from_scaled_val(2))
                .unwrap(),
            Rate::from_scaled_val(0)
        );
        assert_eq!(
            Rate::from_scaled_val(u128::MAX)
                .try_sub(Rate::from_scaled_val(u128::MAX))
                .unwrap(),
            Rate::from_scaled_val(0)
        );
        assert!(Rate::from_scaled_val(0)
            .try_sub(Rate::from_scaled_val(2))
            .is_err());

        assert_eq!(
            Rate::from_scaled_val(WAD as u128 * 2)
                .try_pow(2u64)
                .unwrap(),
            Rate::from_scaled_val(WAD as u128 * 4)
        );
        assert_eq!(
            Rate::from_scaled_val(WAD as u128 * 2)
                .try_pow(3u64)
                .unwrap(),
            Rate::from_scaled_val(WAD as u128 * 8)
        );
        assert_eq!(
            Rate::from_scaled_val(WAD as u128 * 2)
                .try_pow(0u64)
                .unwrap(),
            Rate::one()
        );

        assert_eq!(&format!("{}", Rate::one()), "1.000000000");
        assert_eq!(&format!("{}", Rate::from_scaled_val(2)), "0.000000002");

        assert_eq!(
            Rate::try_from(Decimal::from_scaled_val(2)).unwrap(),
            Rate::from_scaled_val(2)
        );
    }
}
