//! Intelligent Market Maker V1

use super::*;
use crate::{
    error::SwapError,
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
    state::{pack_decimal, unpack_decimal},
};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
};

use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
};

/// Multiplier status enum
#[derive(Clone, Copy, PartialEq, Debug, Hash)]
pub enum Multiplier {
    /// multiplier = 1
    One,
    /// multiplier > 1
    AboveOne,
    /// multiplier < 1
    BelowOne,
}

impl Default for Multiplier {
    fn default() -> Self {
        Self::One
    }
}

impl TryFrom<u8> for Multiplier {
    type Error = ProgramError;

    fn try_from(multiplier: u8) -> Result<Self, Self::Error> {
        match multiplier {
            0 => Ok(Multiplier::One),
            1 => Ok(Multiplier::AboveOne),
            2 => Ok(Multiplier::BelowOne),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}

/// PoolState struct
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoolState {
    /// market price
    pub market_price: Decimal,
    /// slope
    pub slope: Decimal,
    /// base token regression target
    pub base_target: Decimal,
    /// quote token regression target
    pub quote_target: Decimal,
    /// base token reserve
    pub base_reserve: Decimal,
    /// quote token reserve
    pub quote_reserve: Decimal,
    /// Multiplier status
    pub multiplier: Multiplier,
}

impl PoolState {
    /// Create new pool state
    pub fn new(params: PoolState) -> Result<Self, ProgramError> {
        let mut pool = Self::default();
        pool.init(params);
        pool.adjust_target()?;
        Ok(pool)
    }

    /// Init pool state
    pub fn init(&mut self, params: PoolState) {
        self.market_price = params.market_price;
        self.slope = params.slope;
        self.base_target = params.base_target;
        self.base_reserve = params.base_reserve;
        self.quote_target = params.quote_target;
        self.quote_reserve = params.quote_reserve;
        self.multiplier = params.multiplier;
    }

    /// Adjust pool token target.
    ///
    /// # Return value
    ///
    /// adjusted token target.
    pub fn adjust_target(&mut self) -> ProgramResult {
        match self.multiplier {
            Multiplier::AboveOne => {
                self.quote_target = get_target_reserve(
                    self.quote_reserve,
                    self.base_reserve.try_sub(self.base_target)?,
                    self.market_price,
                    self.slope,
                )?
            }
            Multiplier::BelowOne => {
                self.base_target = get_target_reserve(
                    self.base_reserve,
                    self.quote_reserve.try_sub(self.quote_target)?,
                    self.market_price.reciprocal()?,
                    self.slope,
                )?
            }
            _ => {}
        };
        Ok(())
    }

    /// Get adjusted market price based on the current pool status and intelligent
    /// market making curve.
    ///
    /// # Return value
    ///
    /// adjusted market price.
    pub fn get_mid_price(&mut self) -> Result<Decimal, ProgramError> {
        self.adjust_target()?;
        match self.multiplier {
            Multiplier::BelowOne => {
                let multiplier = self
                    .quote_target
                    .try_mul(self.quote_target)?
                    .try_div(self.quote_reserve)?
                    .try_div(self.quote_reserve)?;
                let multiplier = multiplier
                    .try_mul(self.slope)?
                    .try_add(Decimal::one())?
                    .try_sub(self.slope)?;
                self.market_price.try_div(multiplier)
            }
            _ => {
                let multiplier = self
                    .base_target
                    .try_mul(self.base_target)?
                    .try_div(self.base_reserve)?
                    .try_div(self.base_reserve)?;
                let multiplier = multiplier
                    .try_mul(self.slope)?
                    .try_add(Decimal::one())?
                    .try_sub(self.slope)?;
                self.market_price.try_mul(multiplier)
            }
        }
    }

    /// Sell base token for quote token with multiplier input.
    ///
    /// # Arguments
    ///
    /// * base_amount - base amount to sell.
    /// * multiplier - multiplier status.
    ///
    /// # Return value
    ///
    /// purchased quote token amount.
    fn sell_base_token_with_multiplier(
        &self,
        base_amount: Decimal,
        multiplier: Multiplier,
    ) -> Result<Decimal, ProgramError> {
        match multiplier {
            Multiplier::One => get_target_amount_reverse_direction(
                self.quote_target,
                self.quote_target,
                base_amount,
                self.market_price,
                self.slope,
            ),
            Multiplier::AboveOne => get_target_amount(
                self.base_target,
                self.base_reserve.try_add(base_amount)?,
                self.base_reserve,
                self.market_price,
                self.slope,
            ),
            Multiplier::BelowOne => get_target_amount_reverse_direction(
                self.quote_target,
                self.quote_reserve,
                base_amount,
                self.market_price,
                self.slope,
            ),
        }
    }

    /// Sell base token for quote token.
    ///
    /// # Arguments
    ///
    /// * base_amount - base amount to sell.
    ///
    /// # Return value
    ///
    /// purchased quote token amount, updated multiplier.
    pub fn sell_base_token(&self, base_amount: u64) -> Result<(u64, Multiplier), ProgramError> {
        let (quote_amount, new_multiplier) = match self.multiplier {
            Multiplier::One => (
                self.sell_base_token_with_multiplier(base_amount.into(), Multiplier::One)?,
                Multiplier::BelowOne,
            ),
            Multiplier::BelowOne => (
                self.sell_base_token_with_multiplier(base_amount.into(), Multiplier::BelowOne)?,
                Multiplier::BelowOne,
            ),
            Multiplier::AboveOne => {
                let back_to_one_pay_base = self.base_target.try_sub(self.base_reserve)?;
                let back_to_one_receive_quote = self.quote_reserve.try_sub(self.quote_target)?;

                match back_to_one_pay_base.cmp(&Decimal::from(base_amount)) {
                    Ordering::Greater => (
                        self.sell_base_token_with_multiplier(
                            base_amount.into(),
                            Multiplier::AboveOne,
                        )?
                        .min(back_to_one_receive_quote),
                        Multiplier::AboveOne,
                    ),
                    Ordering::Equal => (back_to_one_receive_quote, Multiplier::One),
                    Ordering::Less => (
                        self.sell_base_token_with_multiplier(
                            Decimal::from(base_amount).try_sub(back_to_one_pay_base)?,
                            Multiplier::One,
                        )?
                        .try_add(back_to_one_receive_quote)?,
                        Multiplier::BelowOne,
                    ),
                }
            }
        };
        Ok((quote_amount.try_floor_u64()?, new_multiplier))
    }

    /// Sell quote token for base token with multiplier input.
    ///
    /// # Arguments
    ///
    /// * quote_amount - quote amount to sell.
    /// * multiplier - multiplier status.
    ///
    /// # Return value
    ///
    /// purchased base token amount.
    fn sell_quote_token_with_multiplier(
        &self,
        quote_amount: Decimal,
        multiplier: Multiplier,
    ) -> Result<Decimal, ProgramError> {
        match multiplier {
            Multiplier::One => get_target_amount_reverse_direction(
                self.base_target,
                self.base_target,
                quote_amount,
                self.market_price.reciprocal()?,
                self.slope,
            ),
            Multiplier::AboveOne => get_target_amount_reverse_direction(
                self.base_target,
                self.base_reserve,
                quote_amount,
                self.market_price.reciprocal()?,
                self.slope,
            ),
            Multiplier::BelowOne => get_target_amount(
                self.quote_target,
                self.quote_reserve.try_add(quote_amount)?,
                self.quote_reserve,
                self.market_price.reciprocal()?,
                self.slope,
            ),
        }
    }

    /// Sell quote token for base token.
    ///
    /// # Arguments
    ///
    /// * quote_amount - quote amount to sell.
    ///
    /// # Return value
    ///
    /// purchased base token amount, updated multiplier.
    pub fn sell_quote_token(&self, quote_amount: u64) -> Result<(u64, Multiplier), ProgramError> {
        let (base_amount, new_multiplier) = match self.multiplier {
            Multiplier::One => (
                self.sell_quote_token_with_multiplier(quote_amount.into(), Multiplier::One)?,
                Multiplier::AboveOne,
            ),
            Multiplier::AboveOne => (
                self.sell_quote_token_with_multiplier(quote_amount.into(), Multiplier::AboveOne)?,
                Multiplier::AboveOne,
            ),
            Multiplier::BelowOne => {
                let back_to_one_pay_quote = self.quote_target.try_sub(self.quote_reserve)?;
                let back_to_one_receive_base = self.base_reserve.try_sub(self.base_target)?;

                match back_to_one_pay_quote.cmp(&Decimal::from(quote_amount)) {
                    Ordering::Greater => (
                        self.sell_quote_token_with_multiplier(
                            quote_amount.into(),
                            Multiplier::BelowOne,
                        )?
                        .min(back_to_one_receive_base),
                        Multiplier::BelowOne,
                    ),
                    Ordering::Equal => (back_to_one_receive_base, Multiplier::One),
                    Ordering::Less => (
                        self.sell_quote_token_with_multiplier(
                            Decimal::from(quote_amount).try_sub(back_to_one_pay_quote)?,
                            Multiplier::One,
                        )?
                        .try_add(back_to_one_receive_base)?,
                        Multiplier::AboveOne,
                    ),
                }
            }
        };
        Ok((base_amount.try_floor_u64()?, new_multiplier))
    }

    /// Buy shares [round down]: deposit and calculate shares.
    ///
    /// # Arguments
    ///
    /// * base_balance - base amount to sell.
    /// * quote_balance - quote amount to sell.
    /// * total_supply - total shares amount.
    ///
    /// # Return value
    ///
    /// purchased shares.
    pub fn buy_shares(
        &mut self,
        base_balance: u64,
        quote_balance: u64,
        total_supply: u64,
    ) -> Result<u64, ProgramError> {
        let base_balance = Decimal::from(base_balance);
        let quote_balance = Decimal::from(quote_balance);
        let base_input = base_balance.try_sub(self.base_reserve)?;
        let quote_input = quote_balance.try_sub(self.quote_reserve)?;

        if base_input.is_zero() {
            return Err(SwapError::InsufficientFunds.into());
        }

        let shares = if total_supply == 0 {
            // case 1. initial supply
            let shares = if self.market_price.try_mul(base_balance)? > quote_balance {
                quote_balance.try_div(self.market_price)?
            } else {
                base_balance
            };
            self.base_target = shares;
            self.quote_target = shares.try_mul(self.market_price)?;
            shares
        } else if self.base_reserve > Decimal::zero() && self.quote_reserve > Decimal::zero() {
            // case 2. normal case
            let base_input_ratio = base_input.try_div(self.base_reserve)?;
            let quote_input_ratio = quote_input.try_div(self.quote_reserve)?;
            let mint_ratio = base_input_ratio.min(quote_input_ratio);
            let shares = mint_ratio.try_mul(total_supply)?;

            self.base_target = self
                .base_target
                .try_mul(mint_ratio)?
                .try_add(self.base_target)?;
            self.quote_target = self
                .quote_target
                .try_mul(mint_ratio)?
                .try_add(self.quote_target)?;
            shares
        } else {
            return Err(SwapError::IncorrectMint.into());
        };

        self.base_reserve = base_balance;
        self.quote_reserve = quote_balance;
        shares.try_floor_u64()
    }

    /// Sell shares [round down]: withdraw shares and calculate the withdrawn amount.
    ///
    /// # Arguments
    ///
    /// * share_amount - share amount to sell.
    /// * base_min_amount - base min amount.
    /// * quote_min_amount - quote min amount.
    /// * total_supply - total shares amount.
    ///
    /// # Return value
    ///
    /// base amount, quote amount.
    pub fn sell_shares(
        &mut self,
        share_amount: u64,
        base_min_amount: u64,
        quote_min_amount: u64,
        total_supply: u64,
    ) -> Result<(u64, u64), ProgramError> {
        let base_balance = self.base_reserve;
        let quote_balance = self.quote_reserve;

        let base_amount = base_balance.try_mul(share_amount)?.try_div(total_supply)?;
        let quote_amount = quote_balance.try_mul(share_amount)?.try_div(total_supply)?;

        self.base_target = self.base_target.try_sub(
            self.base_target
                .try_mul(share_amount)?
                .try_div(total_supply)?,
        )?;
        self.quote_target = self.quote_target.try_sub(
            self.quote_target
                .try_mul(share_amount)?
                .try_div(total_supply)?,
        )?;

        if base_amount < Decimal::from(base_min_amount)
            || quote_amount < Decimal::from(quote_min_amount)
        {
            return Err(SwapError::WithdrawNotEnough.into());
        }

        self.base_reserve = self.base_reserve.try_sub(base_amount)?;
        self.quote_reserve = self.quote_reserve.try_sub(quote_amount)?;

        Ok((base_amount.try_floor_u64()?, quote_amount.try_floor_u64()?))
    }

    /// Calculate deposit amount according to the reserve.
    ///
    /// a_reserve = 0 & b_reserve = 0 => (a_amount, b_amount)
    /// a_reserve > 0 & b_reserve = 0 => (a_amount, 0)
    /// a_reserve > 0 & b_reserve > 0 => (a_amount*ratio1, b_amount*ratio2)
    ///
    /// # Arguments
    ///
    /// * base_in_amount - base in amount.
    /// * quote_in_amount - quote in amount.
    ///
    /// # Return value
    ///
    /// base deposit amount, quote deposit amount.
    pub fn calculate_deposit_amount(
        &self,
        base_in_amount: u64,
        quote_in_amount: u64,
    ) -> Result<(u64, u64), ProgramError> {
        let base_in_amount = Decimal::from(base_in_amount);
        let quote_in_amount = Decimal::from(quote_in_amount);

        let (base_in_amount, quote_in_amount) =
            if self.base_reserve.is_zero() && self.quote_reserve.is_zero() {
                let shares = match self
                    .market_price
                    .try_mul(base_in_amount)?
                    .cmp(&quote_in_amount)
                {
                    Ordering::Greater => quote_in_amount.try_div(self.market_price)?,
                    _ => base_in_amount,
                };
                (shares, shares.try_mul(self.market_price)?)
            } else if self.base_reserve > Decimal::zero() && self.quote_reserve > Decimal::zero() {
                let base_increase_ratio = base_in_amount.try_div(self.base_reserve)?;
                let quote_increase_ratio = quote_in_amount.try_div(self.quote_reserve)?;

                if base_increase_ratio < quote_increase_ratio {
                    (
                        base_in_amount,
                        self.quote_reserve.try_mul(base_increase_ratio)?,
                    )
                } else {
                    (
                        self.base_reserve.try_mul(quote_increase_ratio)?,
                        quote_in_amount,
                    )
                }
            } else {
                (base_in_amount, quote_in_amount)
            };

        Ok((
            base_in_amount.try_floor_u64()?,
            quote_in_amount.try_floor_u64()?,
        ))
    }
}

impl Sealed for PoolState {}

/// PoolState packed size
pub const POOL_STATE_SIZE: usize = 97; // 16 + 16 + 16 + 16 + 16 + 16 +1
impl Pack for PoolState {
    const LEN: usize = POOL_STATE_SIZE;
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, POOL_STATE_SIZE];
        let (
            market_price,
            slope,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            multiplier,
        ) = mut_array_refs![output, 16, 16, 16, 16, 16, 16, 1];
        pack_decimal(self.market_price, market_price);
        pack_decimal(self.slope, slope);
        pack_decimal(self.base_reserve, base_reserve);
        pack_decimal(self.quote_reserve, quote_reserve);
        pack_decimal(self.base_target, base_target);
        pack_decimal(self.quote_target, quote_target);
        multiplier[0] = self.multiplier as u8;
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, POOL_STATE_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            market_price,
            slope,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            multiplier,
        ) = array_refs![input, 16, 16, 16, 16, 16, 16, 1];
        Ok(Self {
            market_price: unpack_decimal(market_price),
            slope: unpack_decimal(slope),
            base_reserve: unpack_decimal(base_reserve),
            quote_reserve: unpack_decimal(quote_reserve),
            base_target: unpack_decimal(base_target),
            quote_target: unpack_decimal(quote_target),
            multiplier: multiplier[0].try_into()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let pool_state = PoolState {
            market_price: default_market_price(),
            slope: default_slope(),
            base_target: Decimal::from(1_000_000_000u64),
            quote_target: Decimal::from(500_000_000u64),
            base_reserve: Decimal::from(1_000_000_000u64),
            quote_reserve: Decimal::from(500_000_000u64),
            multiplier: Multiplier::One,
        };

        let mut new_pool_state = PoolState::default();
        new_pool_state.init(pool_state.clone());
        assert_eq!(new_pool_state, pool_state);
    }

    #[test]
    fn test_one_sell_token() {
        let pool_state = PoolState {
            market_price: default_market_price(),
            slope: default_slope(),
            base_target: Decimal::from(1_000_000_000u64),
            quote_target: Decimal::from(1_000_000_000u64),
            base_reserve: Decimal::from(1_000_000_000u64),
            quote_reserve: Decimal::from(1_000_000_000u64),
            multiplier: Multiplier::One,
        };

        let quote_token = pool_state.sell_base_token(100u64).unwrap();
        assert_eq!(quote_token, (10000u64, Multiplier::BelowOne));

        let base_token = pool_state.sell_quote_token(100u64).unwrap();
        assert_eq!(base_token, (1u64, Multiplier::AboveOne));
    }

    #[test]
    fn test_get_mid_price() {
        let mut pool_state = PoolState {
            market_price: default_market_price(),
            slope: default_slope(),
            base_target: Decimal::from(1_000_000_000u64),
            quote_target: Decimal::from(1_000_000_000u64),
            base_reserve: Decimal::from(1_000_000_000u64),
            quote_reserve: Decimal::from(1_000_000_000u64),
            multiplier: Multiplier::One,
        };

        let mid_price = pool_state.get_mid_price().unwrap();
        assert_eq!(mid_price, Decimal::from(100u64));
    }

    #[test]
    fn test_failure() {
        assert_eq!(
            Multiplier::try_from(3u8),
            Err(ProgramError::InvalidAccountData)
        );

        let mut pool_state = PoolState {
            market_price: default_market_price(),
            slope: default_slope(),
            base_target: Decimal::from(1_000_000_000u64),
            quote_target: Decimal::from(500_000_000u64),
            base_reserve: Decimal::from(1_000_000_000u64),
            quote_reserve: Decimal::from(500_000_000u64),
            multiplier: Multiplier::One,
        };
        assert_eq!(
            pool_state.buy_shares(1_000_000_000u64, 500_000_000u64, 1_000_000_000u64),
            Err(SwapError::InsufficientFunds.into())
        );

        pool_state.base_reserve = Decimal::from(0u64);
        assert_eq!(
            pool_state.buy_shares(500_000_000u64, 1_000_000_000u64, 1_000_000_000u64),
            Err(SwapError::IncorrectMint.into())
        );

        pool_state.base_reserve = Decimal::from(1_000_000_000u64);
        assert_eq!(
            pool_state.sell_shares(
                500_000_000u64,
                1_000_000_000u64,
                1_000_000_000u64,
                1_000_000_000u64
            ),
            Err(SwapError::WithdrawNotEnough.into())
        );
    }

    #[test]
    fn test_packing_pool() {
        let pool_state = PoolState {
            market_price: default_market_price(),
            slope: default_slope(),
            base_target: Decimal::from(1_000_000_000u64),
            quote_target: Decimal::from(500_000_000u64),
            base_reserve: Decimal::from(1_000_000_000u64),
            quote_reserve: Decimal::from(500_000_000u64),
            multiplier: Multiplier::One,
        };

        let mut packed = [0u8; PoolState::LEN];
        PoolState::pack_into_slice(&pool_state, &mut packed);
        let unpacked = PoolState::unpack_from_slice(&packed).unwrap();
        assert_eq!(pool_state, unpacked);
    }
}
