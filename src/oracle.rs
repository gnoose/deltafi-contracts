//! Moving Average = Oracle Price on Solana
use std::time::{SystemTime, UNIX_EPOCH};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use log::trace;
use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    pubkey::Pubkey,
};

use crate::bn::U256;

/// Oracle struct
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Oracle {
    /// Period for moving aveage
    pub period: u32,

    /// Program id for token0
    pub token0: Pubkey,

    /// Program id for token1
    pub token1: Pubkey,

    /// cumulative price for token0
    pub price0_cumulative_last: U256,

    /// cumulative price for token1
    pub price1_cumulative_last: U256,

    /// last block timestamp - second
    pub block_timestamp_last: u64,

    /// average price for token0
    price0_average: U256,

    /// average price for token1
    price1_average: U256,
}

impl Oracle {
    /// initialize function for Oracle
    pub fn new(token0: Pubkey, token1: Pubkey) -> Self {
        Self {
            period: 24,
            token0,
            token1,
            price0_cumulative_last: U256::from(0),
            price1_cumulative_last: U256::from(0),
            block_timestamp_last: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            price0_average: U256::from(0),
            price1_average: U256::from(0),
        }
    }

    /// update oracle info by current price info and tiemstamp
    pub fn update(
        &mut self,
        price0_cumulative: U256,
        price1_cumulative: U256,
        block_timestamp: u64,
    ) {
        let time_elapsed = block_timestamp - self.block_timestamp_last;

        if time_elapsed >= self.period as u64 {
            trace!("ExampleOracleSimple: PERIOD_NOT_ELAPSED");
        } else {
            self.price0_average = (price0_cumulative - self.price0_cumulative_last) / time_elapsed;
            self.price1_average = (price1_cumulative - self.price1_cumulative_last) / time_elapsed;

            self.price0_cumulative_last = price0_cumulative;
            self.price1_cumulative_last = price1_cumulative;
            self.block_timestamp_last = block_timestamp;
        }
    }

    /// calc current CumulativePrice from the current token price
    pub fn current_cumulative_price(
        &self,
        price0: U256,
        price1: U256,
        current_timestamp: u64,
    ) -> (U256, U256, u64) {
        let mut price0_cumulative = self.price0_cumulative_last;
        let mut price1_cumulative = self.price1_cumulative_last;

        let mut block_timestamp = self.block_timestamp_last;

        if block_timestamp != current_timestamp {
            let time_elapsed = U256::from(current_timestamp - block_timestamp);

            price0_cumulative = price0 * time_elapsed + price0_cumulative;
            price1_cumulative = price1 * time_elapsed + price1_cumulative;

            block_timestamp = current_timestamp;
        }

        (price0_cumulative, price1_cumulative, block_timestamp)
    }

    /// return the consult of the oracle
    pub fn consult(&self, token: Pubkey, amount_in: U256) -> U256 {
        if token == self.token0 {
            self.price0_average * amount_in
        } else if token == self.token1 {
            self.price1_average * amount_in
        } else {
            U256::from(0)
        }
    }
}

impl Sealed for Oracle {}
impl Pack for Oracle {
    const LEN: usize = 204;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 204];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            period,
            token0,
            token1,
            price0_cumulative_last,
            price1_cumulative_last,
            block_timestamp_last,
            price0_average,
            price1_average,
        ) = array_refs![input, 4, 32, 32, 32, 32, 8, 32, 32];
        Ok(Self {
            period: u32::from_le_bytes(*period),
            token0: Pubkey::new_from_array(*token0),
            token1: Pubkey::new_from_array(*token1),
            price0_cumulative_last: U256::from(price0_cumulative_last),
            price1_cumulative_last: U256::from(price1_cumulative_last),
            block_timestamp_last: u64::from_le_bytes(*block_timestamp_last),
            price0_average: U256::from(price0_average),
            price1_average: U256::from(price1_average),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 204];
        let (
            period,
            token0,
            token1,
            price0_cumulative_last,
            price1_cumulative_last,
            block_timestamp_last,
            price0_average,
            price1_average,
        ) = mut_array_refs![output, 4, 32, 32, 32, 32, 8, 32, 32];
        *period = self.period.to_le_bytes();
        token0.copy_from_slice(self.token0.as_ref());
        token1.copy_from_slice(self.token1.as_ref());
        self.price0_cumulative_last
            .to_little_endian(price0_cumulative_last);
        self.price1_cumulative_last
            .to_little_endian(price1_cumulative_last);
        *block_timestamp_last = self.block_timestamp_last.to_le_bytes();
        self.price0_average.to_little_endian(price0_average);
        self.price1_average.to_little_endian(price1_average);
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use rand::Rng;

    use super::*;

    /* uses */
    /// Timestamp at 0
    pub const ZERO_TS: i64 = 0;

    #[test]
    fn test_current_cumulative_price() {
        let price0: U256 = U256::from(1);
        let price1: U256 = U256::from(1);
        let token0 = Pubkey::default();
        let token1 = Pubkey::default();

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 1;
        let oracle: Oracle = Oracle::new(token0, token1);

        let expected: (U256, U256, u64) = (price0, price1, current_timestamp);

        assert_eq!(
            oracle.current_cumulative_price(price0, price1, current_timestamp),
            expected
        );
    }

    #[test]
    fn test_consult() {
        let token0 = Pubkey::default();
        let token1 = Pubkey::default();
        let token = token0;
        let amount_in = U256::from(rand::thread_rng().gen_range(ZERO_TS, i64::MAX));

        let oracle: Oracle = Oracle::new(token0, token1);

        let expected = U256::from(0);

        assert_eq!(oracle.consult(token, amount_in), expected);
    }
}
