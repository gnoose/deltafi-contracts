//! Curve modules

mod calc;
mod pmm;

pub use calc::*;
pub use pmm::*;

#[cfg(test)]
use crate::math::{Decimal, HALF_WAD};

#[cfg(test)]
/// Slope Value for testing
pub fn default_slop() -> Decimal {
    Decimal::from_scaled_val(HALF_WAD as u128)
}

#[cfg(test)]
/// Market Price for testing
pub fn default_market_price() -> Decimal {
    Decimal::from(100u64)
}

#[cfg(test)]
mod tests {}
