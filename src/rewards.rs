//! Program rewards

use std::cmp::Ordering;

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};

use crate::{
    bn::{FixedU64, U256},
    math::{Decimal, TryDiv, TryMul},
};

/// Rewards structure
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rewards {
    /// Trade reward numerator
    pub trade_reward_numerator: u64,
    /// Trade reward denominator
    pub trade_reward_denominator: u64,
    /// Trade reward cap
    pub trade_reward_cap: u64,
    /// LP reward numerator
    pub liquidity_reward_numerator: u64,
    /// LP reward denominator
    pub liquidity_reward_denominator: u64,
}

impl Rewards {
    /// Create new rewards
    pub fn new(params: &Self) -> Self {
        Rewards {
            trade_reward_numerator: params.trade_reward_numerator,
            trade_reward_denominator: params.trade_reward_denominator,
            trade_reward_cap: params.trade_reward_cap,
            liquidity_reward_numerator: params.liquidity_reward_numerator,
            liquidity_reward_denominator: params.liquidity_reward_denominator,
        }
    }

    /// Calc trade reward amount with [`U256`]
    pub fn trade_reward_u256(&self, amount: U256) -> Result<U256, ProgramError> {
        let c_reward = amount
            .sqrt()?
            .checked_bn_mul(self.trade_reward_numerator.into())?
            .checked_floor_div(self.trade_reward_denominator.into())
            .unwrap();

        match c_reward.cmp(&self.trade_reward_cap.into()) {
            Ordering::Greater => Ok(U256::from(self.trade_reward_cap)),
            _ => Ok(c_reward),
        }
    }

    /// Calc trade reward amount with [`FixedU64`]
    pub fn trade_reward_fixed_u64(&self, amount: FixedU64) -> Result<FixedU64, ProgramError> {
        let c_reward = amount
            .sqrt()?
            .checked_mul_floor(FixedU64::new(self.trade_reward_numerator))?
            .checked_div_floor(FixedU64::new(self.trade_reward_denominator))
            .unwrap();

        match c_reward.into_real_u64_floor().cmp(&self.trade_reward_cap) {
            Ordering::Greater => Ok(FixedU64::new(self.trade_reward_cap)),
            _ => Ok(c_reward),
        }
    }

    /// Calc trade reward amount with [`u64`]
    pub fn trade_reward_u64(&self, amount: u64) -> Result<u64, ProgramError> {
        let c_reward = Decimal::from(amount)
            .sqrt()?
            .try_mul(self.trade_reward_numerator)?
            .try_div(self.trade_reward_denominator)?;

        Ok(if c_reward > Decimal::from(self.trade_reward_cap) {
            self.trade_reward_cap
        } else {
            c_reward.try_floor_u64()?
        })
    }

    /// Calc lp reward amount with [`U256`]
    pub fn liquidity_reward_u256(&self, amount: U256) -> Result<U256, ProgramError> {
        amount
            .checked_bn_mul(self.liquidity_reward_numerator.into())?
            .checked_floor_div(self.liquidity_reward_denominator.into())
    }

    /// Calc lp reward amount with [`FixedU64`]
    pub fn liquidity_reward_fixed_u64(&self, amount: FixedU64) -> Result<FixedU64, ProgramError> {
        amount
            .checked_mul_floor(FixedU64::new(self.liquidity_reward_numerator))?
            .checked_div_floor(FixedU64::new(self.liquidity_reward_denominator))
    }

    /// Calc lp reward amount with [`u64`]
    pub fn liquidity_reward_u64(&self, amount: u64) -> Result<u64, ProgramError> {
        Decimal::from(amount)
            .try_mul(self.liquidity_reward_numerator)?
            .try_div(self.liquidity_reward_denominator)?
            .try_floor_u64()
    }
}

impl Sealed for Rewards {}
impl IsInitialized for Rewards {
    fn is_initialized(&self) -> bool {
        true
    }
}

const REWARDS_SIZE: usize = 40;
impl Pack for Rewards {
    const LEN: usize = REWARDS_SIZE;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, REWARDS_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
            liquidity_reward_numerator,
            liquidity_reward_denominator,
        ) = array_refs![input, 8, 8, 8, 8, 8];
        Ok(Self {
            trade_reward_numerator: u64::from_le_bytes(*trade_reward_numerator),
            trade_reward_denominator: u64::from_le_bytes(*trade_reward_denominator),
            trade_reward_cap: u64::from_le_bytes(*trade_reward_cap),
            liquidity_reward_numerator: u64::from_le_bytes(*liquidity_reward_numerator),
            liquidity_reward_denominator: u64::from_le_bytes(*liquidity_reward_denominator),
        })
    }
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, REWARDS_SIZE];
        let (
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
            liquidity_reward_numerator,
            liquidity_reward_denominator,
        ) = mut_array_refs![output, 8, 8, 8, 8, 8];
        *trade_reward_numerator = self.trade_reward_numerator.to_le_bytes();
        *trade_reward_denominator = self.trade_reward_denominator.to_le_bytes();
        *trade_reward_cap = self.trade_reward_cap.to_le_bytes();
        *liquidity_reward_numerator = self.liquidity_reward_numerator.to_le_bytes();
        *liquidity_reward_denominator = self.liquidity_reward_denominator.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_rewards() {
        let trade_reward_numerator = 1;
        let trade_reward_denominator = 2;
        let trade_reward_cap = 100;
        let liquidity_reward_numerator = 1;
        let liquidity_reward_denominator = 1000;
        let rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
            liquidity_reward_numerator,
            liquidity_reward_denominator,
        };

        let mut packed = [0u8; Rewards::LEN];
        Rewards::pack_into_slice(&rewards, &mut packed[..]);
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);

        let mut packed = vec![];
        packed.extend_from_slice(&trade_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&trade_reward_denominator.to_le_bytes());
        packed.extend_from_slice(&trade_reward_cap.to_le_bytes());
        packed.extend_from_slice(&liquidity_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&liquidity_reward_denominator.to_le_bytes());
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);
    }

    #[test]
    fn reward_results() {
        let trade_reward_numerator = 1;
        let trade_reward_denominator = 2;
        let trade_amount = 100_000_000u64;
        let liquidity_amount = 100_000u64;
        let liquidity_reward_numerator = 1;
        let liquidity_reward_denominator = 1000;

        let mut rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap: 0,
            liquidity_reward_numerator,
            liquidity_reward_denominator,
        };

        // Low reward cap
        {
            let trade_reward_cap = 1_000;
            rewards.trade_reward_cap = trade_reward_cap;

            let expected_trade_reward = U256::from(trade_reward_cap);
            let trade_reward = rewards.trade_reward_u256(trade_amount.into()).unwrap();
            assert_eq!(trade_reward, expected_trade_reward);
            let expected_trade_reward = FixedU64::new(trade_reward_cap.into());
            let trade_reward = rewards
                .trade_reward_fixed_u64(FixedU64::new(trade_amount.into()))
                .unwrap();
            assert_eq!(trade_reward, expected_trade_reward);
        }

        // High reward cap
        {
            let trade_reward_cap = 6_000;
            rewards.trade_reward_cap = trade_reward_cap;

            let expected_trade_reward = 5_000u64;
            let trade_reward = rewards.trade_reward_u256(trade_amount.into()).unwrap();
            assert_eq!(trade_reward, U256::from(expected_trade_reward));
            let trade_reward = rewards
                .trade_reward_fixed_u64(FixedU64::new(trade_amount.into()))
                .unwrap();
            assert_eq!(trade_reward, FixedU64::new(expected_trade_reward.into()));
        }

        // LP reward calc
        {
            let expected_lp_reward = 10u64;
            let lp_reward = rewards
                .liquidity_reward_u256(liquidity_amount.into())
                .unwrap();
            assert_eq!(lp_reward, U256::from(expected_lp_reward));
            let lp_reward = rewards
                .liquidity_reward_fixed_u64(FixedU64::new(liquidity_amount.into()))
                .unwrap();
            assert_eq!(lp_reward, FixedU64::new(expected_lp_reward.into()));
        }
    }
}
