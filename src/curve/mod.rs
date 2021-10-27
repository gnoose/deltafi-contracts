//! Curve modules

mod calc;
mod pmm;

pub use calc::*;
pub use pmm::*;

use crate::math::*;

#[cfg(test)]
/// Slope Value for testing
pub fn default_slop() -> Decimal {
    Decimal::from_scaled_val(HALF_WAD as u128)
}

#[cfg(test)]
/// Mid Price for testing
pub fn default_market_price() -> Decimal {
    Decimal::from(100u64)
}

#[cfg(test)]
mod tests {}
