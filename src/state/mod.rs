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

pub use crate::math::Decimal;

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

#[cfg(test)]
/// Fees for testing
pub const DEFAULT_TEST_FEES: Fees = Fees {
    admin_trade_fee_numerator: 1,
    admin_trade_fee_denominator: 2,
    admin_withdraw_fee_numerator: 1,
    admin_withdraw_fee_denominator: 2,
    trade_fee_numerator: 6,
    trade_fee_denominator: 100,
    withdraw_fee_numerator: 6,
    withdraw_fee_denominator: 100,
};

#[cfg(test)]
/// Rewards for testing
pub const DEFAULT_TEST_REWARDS: Rewards = Rewards {
    trade_reward_numerator: 1,
    trade_reward_denominator: 2,
    trade_reward_cap: 100,
    liquidity_reward_numerator: 1,
    liquidity_reward_denominator: 1000,
};

#[cfg(test)]
mod tests {}
