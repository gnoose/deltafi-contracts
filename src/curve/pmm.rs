//! Proactive Market Maker from dodo

use super::*;
use crate::{
    error::SwapError,
    math::Decimal,
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

/// RStatus enum
#[derive(Clone, Copy, PartialEq, Debug, Hash)]
pub enum RState {
    /// r = 1
    One,
    /// r > 1
    AboveOne,
    /// r < 1
    BelowOne,
}

impl Default for RState {
    fn default() -> Self {
        Self::One
    }
}

impl TryFrom<u8> for RState {
    type Error = ProgramError;

    fn try_from(r: u8) -> Result<Self, Self::Error> {
        match r {
            0 => Ok(RState::One),
            1 => Ok(RState::AboveOne),
            2 => Ok(RState::BelowOne),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}

/// PMMState struct
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PMMState {
    /// mid price
    pub market_price: Decimal,
    /// slop
    pub slop: Decimal,
    /// base token regression target
    pub base_target: Decimal,
    /// quote token regression target
    pub quote_target: Decimal,
    /// base token reserve
    pub base_reserve: Decimal,
    /// quote token reserve
    pub quote_reserve: Decimal,
    /// R status
    pub r: RState,
}

impl PMMState {
    /// Create new PMM state
    pub fn new(params: PMMState) -> Result<Self, ProgramError> {
        let mut pmm = Self::default();
        pmm.init(params);
        pmm.adjust_target()?;
        Ok(pmm)
    }

    /// Init PMM state
    pub fn init(&mut self, params: PMMState) {
        self.market_price = params.market_price;
        self.slop = params.slop;
        self.base_target = params.base_target;
        self.base_reserve = params.base_reserve;
        self.quote_target = params.quote_target;
        self.quote_reserve = params.quote_reserve;
        self.r = params.r;
    }

    // ================================ R = 1 case ====================================

    fn r_one_sell_base_token(&self, base_amount: Decimal) -> Result<Decimal, ProgramError> {
        solve_quadratic_for_trade(
            self.quote_target,
            self.quote_target,
            base_amount,
            self.market_price,
            self.slop,
        )
    }

    fn r_one_sell_quote_token(&self, quote_amount: Decimal) -> Result<Decimal, ProgramError> {
        solve_quadratic_for_trade(
            self.base_target,
            self.base_target,
            quote_amount,
            self.market_price.reciprocal()?,
            self.slop,
        )
    }

    // ================================ R < 1 case ====================================

    fn r_bellow_sell_base_token(&self, base_amount: Decimal) -> Result<Decimal, ProgramError> {
        solve_quadratic_for_trade(
            self.quote_target,
            self.quote_reserve,
            base_amount,
            self.market_price,
            self.slop,
        )
    }

    fn r_bellow_sell_quote_token(&self, quote_amount: Decimal) -> Result<Decimal, ProgramError> {
        general_integrate(
            self.quote_target,
            self.quote_reserve.try_add(quote_amount)?,
            self.quote_reserve,
            self.market_price.reciprocal()?,
            self.slop,
        )
    }

    // ================================ R > 1 case ====================================

    fn r_above_sell_base_token(&self, base_amount: Decimal) -> Result<Decimal, ProgramError> {
        general_integrate(
            self.base_target,
            self.base_reserve.try_add(base_amount)?,
            self.base_reserve,
            self.market_price,
            self.slop,
        )
    }

    fn r_above_sell_quote_token(&self, quote_amount: Decimal) -> Result<Decimal, ProgramError> {
        solve_quadratic_for_trade(
            self.base_target,
            self.base_reserve,
            quote_amount,
            self.market_price.reciprocal()?,
            self.slop,
        )
    }

    // ==================== Helper functions ========================

    /// Perform adjustment target value
    pub fn adjust_target(&mut self) -> ProgramResult {
        match self.r {
            RState::AboveOne => {
                self.quote_target = solve_quadratic_for_target(
                    self.quote_reserve,
                    self.base_reserve.try_sub(self.base_target)?,
                    self.market_price,
                    self.slop,
                )?
            }
            RState::BelowOne => {
                self.base_target = solve_quadratic_for_target(
                    self.base_reserve,
                    self.quote_reserve.try_sub(self.quote_target)?,
                    self.market_price.reciprocal()?,
                    self.slop,
                )?
            }
            _ => return Err(SwapError::Equilibrium.into()),
        };
        Ok(())
    }

    /// Get mid prixe of the current PMM status
    pub fn get_mid_price(&mut self) -> Result<Decimal, ProgramError> {
        self.adjust_target()?;
        match self.r {
            RState::BelowOne => {
                let r = self
                    .quote_target
                    .try_mul(self.quote_target)?
                    .try_div(self.quote_reserve)?
                    .try_div(self.quote_reserve)?;
                let r = r
                    .try_mul(self.slop)?
                    .try_add(Decimal::one())?
                    .try_sub(self.slop)?;
                self.market_price.try_div(r)
            }
            _ => {
                let r = self
                    .base_target
                    .try_mul(self.base_target)?
                    .try_div(self.base_reserve)?
                    .try_div(self.base_reserve)?;
                let r = r
                    .try_mul(self.slop)?
                    .try_add(Decimal::one())?
                    .try_sub(self.slop)?;
                self.market_price.try_mul(r)
            }
        }
    }

    /// Sell base token
    pub fn sell_base_token(&self, base_amount: u64) -> Result<(u64, RState), ProgramError> {
        let (quote_amount, new_r) = match self.r {
            RState::One => (
                self.r_one_sell_base_token(base_amount.into())?,
                RState::BelowOne,
            ),
            RState::BelowOne => (
                self.r_bellow_sell_base_token(base_amount.into())?,
                RState::BelowOne,
            ),
            RState::AboveOne => {
                let back_to_one_pay_base = self.base_target.try_sub(self.base_reserve)?;
                let back_to_one_receive_quote = self.quote_reserve.try_sub(self.quote_target)?;

                match back_to_one_pay_base.cmp(&Decimal::from(base_amount)) {
                    Ordering::Greater => (
                        self.r_above_sell_base_token(base_amount.into())?
                            .min(back_to_one_receive_quote),
                        RState::AboveOne,
                    ),
                    Ordering::Equal => (back_to_one_receive_quote, RState::One),
                    Ordering::Less => (
                        self.r_one_sell_base_token(
                            Decimal::from(base_amount).try_sub(back_to_one_pay_base)?,
                        )?
                        .try_add(back_to_one_receive_quote)?,
                        RState::BelowOne,
                    ),
                }
            }
        };
        Ok((quote_amount.try_floor_u64()?, new_r))
    }

    /// Sell quote token
    pub fn sell_quote_token(&self, quote_amount: u64) -> Result<(u64, RState), ProgramError> {
        let (base_amount, new_r) = match self.r {
            RState::One => (
                self.r_one_sell_quote_token(quote_amount.into())?,
                RState::AboveOne,
            ),
            RState::AboveOne => (
                self.r_above_sell_quote_token(quote_amount.into())?,
                RState::AboveOne,
            ),
            RState::BelowOne => {
                let back_to_one_pay_quote = self.quote_target.try_sub(self.quote_reserve)?;
                let back_to_one_receive_base = self.base_reserve.try_sub(self.base_target)?;

                match back_to_one_pay_quote.cmp(&Decimal::from(quote_amount)) {
                    Ordering::Greater => (
                        self.r_bellow_sell_quote_token(quote_amount.into())?
                            .min(back_to_one_receive_base),
                        RState::BelowOne,
                    ),
                    Ordering::Equal => (back_to_one_receive_base, RState::One),
                    Ordering::Less => (
                        self.r_one_sell_quote_token(
                            Decimal::from(quote_amount).try_sub(back_to_one_pay_quote)?,
                        )?
                        .try_add(back_to_one_receive_base)?,
                        RState::AboveOne,
                    ),
                }
            }
        };
        Ok((base_amount.try_floor_u64()?, new_r))
    }

    /// Buy shares [round down]
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
            return Err(SwapError::NoBaseInput.into());
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

    /// Sell shares [round down]
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

    /// Calculate deposit amount according to the reserve amount
    ///      a_reserve = 0 & b_reserve = 0 => (a_amount, b_amount)
    ///      a_reserve > 0 & b_reserve = 0 => (a_amount, 0)
    ///      a_reserve > 0 & b_reserve > 0 => (a_amount*ratio1, b_amount*ratio2)
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

impl Sealed for PMMState {}

/// PMMState packed size
pub const PMM_STATE_SIZE: usize = 97; // 16 + 16 + 16 + 16 + 16 + 16 +1
impl Pack for PMMState {
    const LEN: usize = PMM_STATE_SIZE;
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, PMM_STATE_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (market_price, slop, base_reserve, quote_reserve, base_target, quote_target, r) =
            array_refs![input, 16, 16, 16, 16, 16, 16, 1];
        Ok(Self {
            market_price: unpack_decimal(market_price),
            slop: unpack_decimal(slop),
            base_reserve: unpack_decimal(base_reserve),
            quote_reserve: unpack_decimal(quote_reserve),
            base_target: unpack_decimal(base_target),
            quote_target: unpack_decimal(quote_target),
            r: r[0].try_into()?,
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, PMM_STATE_SIZE];
        let (market_price, slop, base_reserve, quote_reserve, base_target, quote_target, r) =
            mut_array_refs![output, 16, 16, 16, 16, 16, 16, 1];
        pack_decimal(self.market_price, market_price);
        pack_decimal(self.slop, slop);
        pack_decimal(self.base_reserve, base_reserve);
        pack_decimal(self.quote_reserve, quote_reserve);
        pack_decimal(self.base_target, base_target);
        pack_decimal(self.quote_target, quote_target);
        r[0] = self.r as u8;
    }
}
