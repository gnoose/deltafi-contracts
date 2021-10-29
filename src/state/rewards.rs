//! Program rewards

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};

use crate::math::{Decimal, TryDiv, TryMul};

/// Rewards structure
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
    ///
    /// # Arguments
    ///
    /// * params - rewards params.
    ///
    /// # Return value
    ///
    /// rewards constructed.
    pub fn new(params: &Self) -> Self {
        Rewards {
            trade_reward_numerator: params.trade_reward_numerator,
            trade_reward_denominator: params.trade_reward_denominator,
            trade_reward_cap: params.trade_reward_cap,
            liquidity_reward_numerator: params.liquidity_reward_numerator,
            liquidity_reward_denominator: params.liquidity_reward_denominator,
        }
    }

    /// Calc trade reward amount with [`u64`]
    ///
    /// # Arguments
    ///
    /// * amount - trade amount.
    ///
    /// # Return value
    ///
    /// trade reward.
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

    /// Calc lp reward amount with [`u64`]
    ///
    /// # Arguments
    ///
    /// * amount - liquidity amount.
    ///
    /// # Return value
    ///
    /// liquidity reward.
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
    use crate::state::DEFAULT_TEST_REWARDS;

    #[test]
    fn pack_rewards() {
        let rewards = DEFAULT_TEST_REWARDS;

        let mut packed = [0u8; Rewards::LEN];
        Rewards::pack_into_slice(&rewards, &mut packed[..]);
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);

        let mut packed = vec![];
        packed.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        packed.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());
        packed.extend_from_slice(&rewards.liquidity_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&rewards.liquidity_reward_denominator.to_le_bytes());
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

            let expected_trade_reward = trade_reward_cap;
            let trade_reward = rewards.trade_reward_u64(trade_amount).unwrap();
            assert_eq!(trade_reward, expected_trade_reward);
        }

        // High reward cap
        {
            let trade_reward_cap = 6_000;
            rewards.trade_reward_cap = trade_reward_cap;

            let expected_trade_reward = 5_000u64;
            let trade_reward = rewards.trade_reward_u64(trade_amount).unwrap();
            assert_eq!(trade_reward, expected_trade_reward);
        }

        // LP reward calc
        {
            let expected_lp_reward = 100u64;
            let lp_reward = rewards.liquidity_reward_u64(liquidity_amount).unwrap();
            assert_eq!(lp_reward, expected_lp_reward);
        }
    }
}
