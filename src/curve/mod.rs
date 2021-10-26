//! Curve modules

mod calc;
mod pmm;

pub use calc::*;
pub use pmm::*;

use crate::math::*;

#[cfg(test)]
/// Slope Value for testing
pub fn default_slop() -> Result<Decimal, solana_program::program_error::ProgramError> {
    Ok(Decimal::one().try_mul(5)?.try_div(10)?)
}

#[cfg(test)]
/// Mid Price for testing
pub fn default_market_price() -> Decimal {
    Decimal::from(100u64)
}

#[cfg(test)]
mod tests {}
