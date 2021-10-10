//! Program rewards

use std::cmp::Ordering;

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
};

use crate::bn::{FixedU64, U256};

/// Rewards structure
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rewards {
    /// Trade reward numerator
    pub trade_reward_numerator: u64,
    /// Trade reward denominator
    pub trade_reward_denominator: u64,
    /// Trade reward cap
    pub trade_reward_cap: u64,
}

impl Rewards {
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

    /// Calc trade reward amount with [`FixedU256`]
    pub fn trade_reward_fixed_u256(&self, amount: FixedU64) -> Result<FixedU64, ProgramError> {
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
}

impl Sealed for Rewards {}
impl Pack for Rewards {
    const LEN: usize = 24;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 24];
        #[allow(clippy::ptr_offset_with_cast)]
        let (trade_reward_numerator, trade_reward_denominator, trade_reward_cap) =
            array_refs![input, 8, 8, 8];
        Ok(Self {
            trade_reward_numerator: u64::from_le_bytes(*trade_reward_numerator),
            trade_reward_denominator: u64::from_le_bytes(*trade_reward_denominator),
            trade_reward_cap: u64::from_le_bytes(*trade_reward_cap),
        })
    }
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 24];
        let (trade_reward_numerator, trade_reward_denominator, trade_reward_cap) =
            mut_array_refs![output, 8, 8, 8];
        *trade_reward_numerator = self.trade_reward_numerator.to_le_bytes();
        *trade_reward_denominator = self.trade_reward_denominator.to_le_bytes();
        *trade_reward_cap = self.trade_reward_cap.to_le_bytes();
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
        let rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
        };

        let mut packed = [0u8; Rewards::LEN];
        Rewards::pack_into_slice(&rewards, &mut packed[..]);
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);

        let mut packed = vec![];
        packed.extend_from_slice(&trade_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&trade_reward_denominator.to_le_bytes());
        packed.extend_from_slice(&trade_reward_cap.to_le_bytes());
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);
    }

    #[test]
    fn reward_results() {
        let trade_reward_numerator = 1;
        let trade_reward_denominator = 2;
        let trade_amount = 100_000_000u64;

        // Low reward cap
        {
            let trade_reward_cap = 1_000;
            let rewards = Rewards {
                trade_reward_numerator,
                trade_reward_denominator,
                trade_reward_cap,
            };

            let expected_trade_reward = U256::from(trade_reward_cap);
            let trade_reward = rewards.trade_reward_u256(trade_amount.into()).unwrap();
            assert_eq!(trade_reward, expected_trade_reward);
            let expected_trade_reward = FixedU64::new(trade_reward_cap.into());
            let trade_reward = rewards
                .trade_reward_fixed_u256(FixedU64::new(trade_amount.into()))
                .unwrap();
            assert_eq!(trade_reward, expected_trade_reward);
        }

        // High reward cap
        {
            let trade_reward_cap = 6_000;
            let rewards = Rewards {
                trade_reward_numerator,
                trade_reward_denominator,
                trade_reward_cap,
            };

            let expected_trade_reward = 5_000u64;
            let trade_reward = rewards.trade_reward_u256(trade_amount.into()).unwrap();
            assert_eq!(trade_reward, U256::from(expected_trade_reward));
            let trade_reward = rewards
                .trade_reward_fixed_u256(FixedU64::new(trade_amount.into()))
                .unwrap();
            assert_eq!(trade_reward, FixedU64::new(expected_trade_reward.into()));
        }
    }
}
