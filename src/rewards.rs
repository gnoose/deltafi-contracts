//! Program rewards

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
};

use crate::bn::U256;

/// Rewards structure
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rewards {
    /// Trade reward numerator
    pub trade_reward_numerator: u64,
    /// Trade reward denominator
    pub trade_reward_denominator: u64,
}

impl Rewards {
    /// Apply trade reward amount
    pub fn trade_reward(&self, trade_amount: U256) -> Option<U256> {
        trade_amount
            .checked_mul(self.trade_reward_numerator.into())?
            .checked_div(self.trade_reward_denominator.into())
    }
}

impl Sealed for Rewards {}
impl Pack for Rewards {
    const LEN: usize = 16;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 16];
        #[allow(clippy::ptr_offset_with_cast)]
        let (trade_reward_numerator, trade_reward_denominator) = array_refs![input, 8, 8];
        Ok(Self {
            trade_reward_numerator: u64::from_le_bytes(*trade_reward_numerator),
            trade_reward_denominator: u64::from_le_bytes(*trade_reward_denominator),
        })
    }
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 16];
        let (trade_reward_numerator, trade_reward_denominator) = mut_array_refs![output, 8, 8];
        *trade_reward_numerator = self.trade_reward_numerator.to_le_bytes();
        *trade_reward_denominator = self.trade_reward_denominator.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_rewards() {
        let trade_reward_numerator = 1;
        let trade_reward_denominator = 2;
        let rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
        };

        let mut packed = [0u8; Rewards::LEN];
        Rewards::pack_into_slice(&rewards, &mut packed[..]);
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);

        let mut packed = vec![];
        packed.extend_from_slice(&trade_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&trade_reward_denominator.to_le_bytes());
        let unpacked = Rewards::unpack_from_slice(&packed).unwrap();
        assert_eq!(rewards, unpacked);
    }

    #[test]
    fn reward_results() {
        let trade_reward_numerator = 1;
        let trade_reward_denominator = 2;
        let rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
        };

        let trade_amount = 1_000_000_000;
        let expected_trade_reward =
            trade_amount * trade_reward_numerator / trade_reward_denominator;
        let trade_fee = rewards.trade_reward(trade_amount.into()).unwrap();
        assert_eq!(trade_fee, expected_trade_reward.into());
    }
}
