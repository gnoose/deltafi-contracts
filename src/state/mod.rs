//! State used in DeFi

mod config;
mod fees;
mod liquidity;
mod rewards;
mod swap;

pub use config::*;
pub use fees::*;
pub use liquidity::*;
pub use rewards::*;
pub use swap::*;

pub use crate::math::*;

use solana_program::program_error::ProgramError;

/// Pack decimal
pub fn pack_decimal(decimal: Decimal, dst: &mut [u8; 16]) {
    *dst = decimal
        .to_scaled_val()
        .expect("Decimal cannot be packed")
        .to_le_bytes();
}

/// Unpack decimal
pub fn unpack_decimal(src: &[u8; 16]) -> Decimal {
    Decimal::from_scaled_val(u128::from_le_bytes(*src))
}

/// Pack boolean
pub fn pack_bool(boolean: bool, dst: &mut [u8; 1]) {
    *dst = (boolean as u8).to_le_bytes()
}

/// Unpack boolean
pub fn unpack_bool(src: &[u8; 1]) -> Result<bool, ProgramError> {
    match u8::from_le_bytes(*src) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProgramError::InvalidAccountData),
    }
}
