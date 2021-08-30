//! Moving Average = Oracle Price on Solana
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError
};
use log::{ trace };
use arrayref::{ array_ref, array_refs };
use std::time::{SystemTime, UNIX_EPOCH};
use crate::{bn::U256, fees::Fees};

/// Oracle struct
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oracle {
    // Period for moving aveage
    pub PERIOD: u32,

    // Program id for token0
    pub token0: Pubkey,

    // Program id for token1
    pub token1: Pubkey,

    // cumulative price for token0
    pub price0CumulativeLast: U256,

    // cumulative price for token1
    pub price1CumulativeLast: U256,

    // last block timestamp
    pub blockTimestampLast: u64,

    // average price for token0
    price0Average: U256,

    // average price for token1
    price1Average: U256,
}

impl Oracle {
    pub fn new(
        token0: Pubkey,
        token1: Pubkey,
    ) -> Self {
        Self {
            PERIOD: 24,
            token0: Pubkey::from(token0),
            token1: Pubkey::from(token1),
            price0CumulativeLast: U256::from(0),
            price1CumulativeLast: U256::from(0),
            blockTimestampLast: u64::from(SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backward")),
            price0Average: U256::from(0),
            price1Average: U256::from(0),
        }
    }

    pub fn update(
        &mut self,
        price0Cumulative: U256,
        price1Cumulative: U256,
        blockTimestamp: u64
    ) {
        let timeElapsed = blockTimestamp - self.blockTimestampLast; // overflow is desired

        // ensure that at least one full period has passed since the last update
        if timeElapsed >= self.PERIOD as u64 {
            trace!("ExampleOracleSimple: PERIOD_NOT_ELAPSED");
        } else {
            // overflow is desired, casting never truncates
            // cumulative price is in (uq112x112 price * seconds) units so we simply wrap it after division by time elapsed
            self.price0Average = f64::from(price0Cumulative.checked_sub(self.price0CumulativeLast)?).checked_div(timeElapsed)?;
            self.price1Average = f64::from(price1Cumulative.checked_sub(self.price1CumulativeLast)?).checked_div(timeElapsed)?;

            self.price0CumulativeLast = price0Cumulative?;
            self.price1CumulativeLast = price1Cumulative?;
            self.blockTimestampLast = blockTimestamp?;
        }
    }

    pub fn currentCumulativePrice(
        &self,
        price0: U256,
        price1: U256,
        currentTimestamp: u64,
    ) -> (U256, U256, u64) {
        let mut price0Cumulative = self.price0CumulativeLast;
        let mut price1Cumulative = self.price1CumulativeLast;

        let mut blockTimestamp = self.blockTimestampLast;

        if blockTimestamp != currentTimestamp {
            //caculate current cumulative price
            let timeElapsed = currentTimestamp - blockTimestamp;

            price0Cumulative = U256::from(price0.checked_mul(timeElapsed as U256)?).checked_add(price0Cumulative)?;
            price1Cumulative = U256::from(price1.checked_mul(timeElapsed as U256)?).checked_add(price1Cumulative)?;

            blockTimestamp = currentTimestamp;
        }

        (price0Cumulative, price1Cumulative, blockTimestamp)
    }

    pub fn consult(
        &self,
        token: &Pubkey,
        amountIn: U256
    ) -> Option<U256> {
        if token == self.token0 {
            self.price0Average.checked_mul(amountIn)
        } else if token == self.token1 {
            self.price1Average.checked_mul(amountIn)
        } else {
            0.into()
        }
    }

    pub fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 204];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            PERIOD,
            token0,
            token1,
            price0CumulativeLast,
            price1CumulativeLast,
            blockTimestampLast,
            price0Average,
            price1Average,
        ) = array_refs![input, 4, 32, 32, 32, 32, 8, 32, 32];
        Ok(Self {
            PERIOD: u32::from(*PERIOD),
            token0: Pubkey::new_from_array(*token0),
            token1: Pubkey::new_from_array(*token1),
            price0CumulativeLast: U256::from(price0CumulativeLast),
            price1CumulativeLast: U256::from(price1CumulativeLast),
            blockTimestampLast: u64::from(blockTimestampLast),
            price0Average: U256::from(price0Average),
            price1Average: U256::from(price1Average),
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::cmp;
    use rand::seq::IteratorRandom;
    use std::time::SystemTime;

    /* uses */
    /// Timestamp at 0
    pub const ZERO_TS: i64 = 0;
    /// Minimum ramp duration
    pub const MIN_RAMP_DURATION: i64 = 86400;
    /// Min amplification coefficient
    pub const MIN_AMP: u64 = 1;
    /// Max amplification coefficient
    pub const MAX_AMP: u64 = 1_000_000;

    #[test]
    fn test_currentCumulativePrice() {
        let price0: U256 = U256::from(1);
        let price1: U256 = U256::from(1);

        let currentTimestamp = SystemTime::now().checked_add(1.into());
        let timeElapsed = 1;
        let oracle: Oracle = Oracle::new(token0, token1)?;

        let expected: (U256, U256, u64) = (price0, price1, u64::from(currentTimestamp))?;

        assert_eq!(
          oracle.currentCumulativePrice(price0, price1, u64::from(currentTimestamp)),
          expected
        );
    }

    #[test]
    fn test_consult() {
        let token = rand::thread_rng().choose(&mut toke0, &mut toke1, &mut rand::thread_rng().gen_range(MIN_AMP, MAX_AMP))?;
        let amountIn = rand::thread_rng().gen_range(ZERO_TS, i64::MAX)?;

        let oracle: Oracle = Oracle::new(token0, token1)?;

        let expected: u64 = 0.into()?;

        assert_eq!(
            oracle.consult(token, amountIn),
            expected
        );

    }
}