//! Math for preserving precision

// required for clippy
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]

mod approximations;
mod decimal;
mod rate;

pub use approximations::*;
pub use decimal::*;
pub use rate::*;

use solana_program::program_error::ProgramError;

/// Scale of precision
pub const SCALE: usize = 9;
/// Identity
pub const WAD: u64 = 1_000_000_000;
/// Half of identity
pub const HALF_WAD: u64 = 500_000_000;
/// Scale for percentages
pub const PERCENT_SCALER: u64 = 10_000_000;

/// Try to subtract, return an error on underflow
pub trait TrySub: Sized {
    /// Subtract
    fn try_sub(self, rhs: Self) -> Result<Self, ProgramError>;
}

/// Try to subtract, return an error on overflow
pub trait TryAdd: Sized {
    /// Add
    fn try_add(self, rhs: Self) -> Result<Self, ProgramError>;
}

/// Try to divide, return an error on overflow or divide by zero
pub trait TryDiv<RHS>: Sized {
    /// Divide
    fn try_div(self, rhs: RHS) -> Result<Self, ProgramError>;
}

/// Try to multiply, return an error on overflow
pub trait TryMul<RHS>: Sized {
    /// Multiply
    fn try_mul(self, rhs: RHS) -> Result<Self, ProgramError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_constants() {
        let base_num: u64 = 10;
        let base_scale: u32 = SCALE as u32;
        assert_eq!(base_num.pow(base_scale), WAD);
        assert_eq!(base_num.pow(base_scale) / 2, HALF_WAD);
        assert_eq!(base_num.pow(base_scale - 2), PERCENT_SCALER);
    }
}
