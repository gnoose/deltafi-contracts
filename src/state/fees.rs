//! Program fees

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};

use crate::error::SwapError;

/// Fees struct
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fees {
    /// Admin trade fee numerator
    pub admin_trade_fee_numerator: u64,
    /// Admin trade fee denominator
    pub admin_trade_fee_denominator: u64,
    /// Admin withdraw fee numerator
    pub admin_withdraw_fee_numerator: u64,
    /// Admin withdraw fee denominator
    pub admin_withdraw_fee_denominator: u64,
    /// Trade fee numerator
    pub trade_fee_numerator: u64,
    /// Trade fee denominator
    pub trade_fee_denominator: u64,
    /// Withdraw fee numerator
    pub withdraw_fee_numerator: u64,
    /// Withdraw fee denominator
    pub withdraw_fee_denominator: u64,
}

impl Fees {
    /// Constructor to create new fees
    ///
    /// # Arguments
    ///
    /// * params - fee parameters.
    ///
    /// # Return value
    ///
    /// fees
    pub fn new(params: &Self) -> Self {
        Fees {
            admin_trade_fee_numerator: params.admin_trade_fee_numerator,
            admin_trade_fee_denominator: params.admin_trade_fee_denominator,
            admin_withdraw_fee_numerator: params.admin_withdraw_fee_numerator,
            admin_withdraw_fee_denominator: params.admin_withdraw_fee_denominator,
            trade_fee_numerator: params.trade_fee_numerator,
            trade_fee_denominator: params.trade_fee_denominator,
            withdraw_fee_numerator: params.withdraw_fee_numerator,
            withdraw_fee_denominator: params.withdraw_fee_denominator,
        }
    }

    /// Apply admin trade fee
    ///
    /// # Arguments
    ///
    /// * fee_amount - fee amount.
    ///
    /// # Return value
    ///
    /// admin trade fee
    pub fn admin_trade_fee(&self, fee_amount: u64) -> Result<u64, ProgramError> {
        fee_amount
            .checked_mul(self.admin_trade_fee_numerator)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(self.admin_trade_fee_denominator)
            .ok_or_else(|| SwapError::CalculationFailure.into())
    }

    /// Apply admin withdraw fee
    ///
    /// # Arguments
    ///
    /// * fee_amount - fee amount.
    ///
    /// # Return value
    ///
    /// admin withdraw fee
    pub fn admin_withdraw_fee(&self, fee_amount: u64) -> Result<u64, ProgramError> {
        fee_amount
            .checked_mul(self.admin_withdraw_fee_numerator)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(self.admin_withdraw_fee_denominator)
            .ok_or_else(|| SwapError::CalculationFailure.into())
    }

    /// Compute trade fee from amount
    ///
    /// # Arguments
    ///
    /// * trade_amount - trade amount.
    ///
    /// # Return value
    ///
    /// trade fee
    pub fn trade_fee(&self, trade_amount: u64) -> Result<u64, ProgramError> {
        trade_amount
            .checked_mul(self.trade_fee_numerator)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(self.trade_fee_denominator)
            .ok_or_else(|| SwapError::CalculationFailure.into())
    }

    /// Compute withdraw fee from amount
    ///
    /// # Arguments
    ///
    /// * withdraw_amount - withdraw amount.
    ///
    /// # Return value
    ///
    /// withdraw fee
    pub fn withdraw_fee(&self, withdraw_amount: u64) -> Result<u64, ProgramError> {
        withdraw_amount
            .checked_mul(self.withdraw_fee_numerator)
            .ok_or(SwapError::CalculationFailure)?
            .checked_div(self.withdraw_fee_denominator)
            .ok_or_else(|| SwapError::CalculationFailure.into())
    }
}

impl Sealed for Fees {}
impl IsInitialized for Fees {
    fn is_initialized(&self) -> bool {
        true
    }
}

const FEES_SIZE: usize = 64;
impl Pack for Fees {
    const LEN: usize = FEES_SIZE;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, FEES_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            admin_trade_fee_numerator,
            admin_trade_fee_denominator,
            admin_withdraw_fee_numerator,
            admin_withdraw_fee_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            withdraw_fee_numerator,
            withdraw_fee_denominator,
        ) = array_refs![input, 8, 8, 8, 8, 8, 8, 8, 8];
        Ok(Self {
            admin_trade_fee_numerator: u64::from_le_bytes(*admin_trade_fee_numerator),
            admin_trade_fee_denominator: u64::from_le_bytes(*admin_trade_fee_denominator),
            admin_withdraw_fee_numerator: u64::from_le_bytes(*admin_withdraw_fee_numerator),
            admin_withdraw_fee_denominator: u64::from_le_bytes(*admin_withdraw_fee_denominator),
            trade_fee_numerator: u64::from_le_bytes(*trade_fee_numerator),
            trade_fee_denominator: u64::from_le_bytes(*trade_fee_denominator),
            withdraw_fee_numerator: u64::from_le_bytes(*withdraw_fee_numerator),
            withdraw_fee_denominator: u64::from_le_bytes(*withdraw_fee_denominator),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, FEES_SIZE];
        let (
            admin_trade_fee_numerator,
            admin_trade_fee_denominator,
            admin_withdraw_fee_numerator,
            admin_withdraw_fee_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            withdraw_fee_numerator,
            withdraw_fee_denominator,
        ) = mut_array_refs![output, 8, 8, 8, 8, 8, 8, 8, 8];
        *admin_trade_fee_numerator = self.admin_trade_fee_numerator.to_le_bytes();
        *admin_trade_fee_denominator = self.admin_trade_fee_denominator.to_le_bytes();
        *admin_withdraw_fee_numerator = self.admin_withdraw_fee_numerator.to_le_bytes();
        *admin_withdraw_fee_denominator = self.admin_withdraw_fee_denominator.to_le_bytes();
        *trade_fee_numerator = self.trade_fee_numerator.to_le_bytes();
        *trade_fee_denominator = self.trade_fee_denominator.to_le_bytes();
        *withdraw_fee_numerator = self.withdraw_fee_numerator.to_le_bytes();
        *withdraw_fee_denominator = self.withdraw_fee_denominator.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DEFAULT_TEST_FEES;

    #[test]
    fn pack_fees() {
        let fees = DEFAULT_TEST_FEES;

        let mut packed = [0u8; Fees::LEN];
        Pack::pack_into_slice(&fees, &mut packed[..]);
        let unpacked = Fees::unpack_from_slice(&packed).unwrap();
        assert_eq!(fees, unpacked);

        let mut packed = vec![];
        packed.extend_from_slice(&fees.admin_trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&fees.admin_trade_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&fees.admin_withdraw_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&fees.admin_withdraw_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&fees.trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&fees.trade_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&fees.withdraw_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&fees.withdraw_fee_denominator.to_le_bytes());
        let unpacked = Fees::unpack_from_slice(&packed).unwrap();
        assert_eq!(fees, unpacked);
    }

    #[test]
    fn fee_results() {
        let fees = DEFAULT_TEST_FEES;

        let trade_amount = 1_000_000_000;
        let expected_trade_fee =
            trade_amount * fees.trade_fee_numerator / fees.trade_fee_denominator;
        let trade_fee = fees.trade_fee(trade_amount).unwrap();
        assert_eq!(trade_fee, expected_trade_fee);
        let expected_admin_trade_fee =
            expected_trade_fee * fees.admin_trade_fee_numerator / fees.admin_trade_fee_denominator;
        assert_eq!(
            fees.admin_trade_fee(trade_fee).unwrap(),
            expected_admin_trade_fee
        );

        let withdraw_amount = 100_000_000_000;
        let expected_withdraw_fee =
            withdraw_amount * fees.withdraw_fee_numerator / fees.withdraw_fee_denominator;
        let withdraw_fee = fees.withdraw_fee(withdraw_amount).unwrap();
        assert_eq!(withdraw_fee, expected_withdraw_fee);
        let expected_admin_withdraw_fee = expected_withdraw_fee * fees.admin_withdraw_fee_numerator
            / fees.admin_withdraw_fee_denominator;
        assert_eq!(
            fees.admin_withdraw_fee(expected_withdraw_fee).unwrap(),
            expected_admin_withdraw_fee
        );
    }
}
