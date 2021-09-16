//! Implement pricing of PMM
use crate::{
    bn::FixedU256,
    math::{
        general_integrate, solve_quadratic_function_for_target, solve_quadratic_function_for_trade,
    },
};

/// RStatus enum
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RStatus {
    /// r = 1
    One,

    /// r > 1
    AboveOne,

    /// r < 1
    BelowOne,
}

impl Default for RStatus {
    fn default() -> Self {
        RStatus::One
    }
}

/// V1curve struct
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V1curve {
    /// slope variable
    pub k: FixedU256,

    /// r status
    pub r_status: RStatus,

    /// oracle price
    pub oracle: FixedU256,

    /// base token balance
    pub base_balance: FixedU256,

    /// quote token balance
    pub quote_balance: FixedU256,

    /// target base token amount
    pub target_base_token_amount: FixedU256,

    /// target quote token amount
    pub target_quote_token_amount: FixedU256,
}

impl V1curve {
    /// initialize function for V1curve
    pub fn new(
        k: FixedU256,
        r_status: RStatus,
        oracle: FixedU256,
        base_balance: FixedU256,
        quote_balance: FixedU256,
        target_base_token_amount: FixedU256,
        target_quote_token_amount: FixedU256,
    ) -> Self {
        Self {
            k,
            r_status,
            oracle,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
        }
    }

    // ================== R = 1 cases ==================
    /// return receiveQuoteToken
    pub fn r_one_sell_base_token(
        &self,
        amount: FixedU256,
        target_quote_token_amount: FixedU256,
    ) -> Option<FixedU256> {
        let i = self.oracle;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_token_amount,
            target_quote_token_amount,
            i.checked_mul_floor(amount)?,
            false,
            self.k,
        )?;

        // in theory Q2 <= target_quote_token_amount
        // however when amount is close to 0, precision problems may cause Q2 > target_quote_token_amount

        target_quote_token_amount.checked_sub(q2)
    }

    /// return payQuoteToken
    pub fn r_one_buy_base_token(
        &self,
        amount: FixedU256,
        target_base_token_amount: FixedU256,
    ) -> Option<FixedU256> {
        let b2 = target_base_token_amount.checked_sub(amount)?;

        self.r_above_integrate(target_base_token_amount, target_base_token_amount, b2)
    }

    // ============ R < 1 cases ============

    /// return receieQuoteToken
    pub fn r_below_sell_base_token(
        &self,
        amount: FixedU256,
        quote_balance: FixedU256,
        target_quote_amount: FixedU256,
    ) -> Option<FixedU256> {
        let i = self.oracle;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            i.checked_mul_floor(amount)?,
            false,
            self.k,
        )?;

        quote_balance.checked_sub(q2)
    }

    /// return payQuoteToken
    pub fn r_below_buy_base_token(
        &self,
        amount: FixedU256,
        quote_balance: FixedU256,
        target_quote_amount: FixedU256,
    ) -> Option<FixedU256> {
        // Here we don't require amount less than some value
        // Because it is limited at upper function

        let i = self.oracle;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            i.checked_mul_ceil(amount)?,
            true,
            self.k,
        )?;

        q2.checked_sub(quote_balance)
    }

    /// return payQuoteToken
    pub fn r_below_back_to_one(&self) -> Option<FixedU256> {
        // important: carefully design the system to make sure spareBase always greater than or equal to 0

        let spare_base = self
            .base_balance
            .checked_sub(self.target_base_token_amount)?;
        let price = self.oracle;
        let fair_amount = spare_base.checked_mul_floor(price)?;
        let new_target_quote =
            solve_quadratic_function_for_target(self.quote_balance, self.k, fair_amount)?;

        new_target_quote.checked_sub(self.quote_balance)
    }

    // ============ R > 1 cases ============

    /// return payQuoteToken
    pub fn r_above_buy_base_token(
        &self,
        amount: FixedU256,
        base_balance: FixedU256,
        target_base_amount: FixedU256,
    ) -> Option<FixedU256> {
        //require(amount < baseBalance, "DODO_BASE_BALANCE_NOT_ENOUGH");

        let b2 = base_balance.checked_sub(amount)?;

        self.r_above_integrate(target_base_amount, base_balance, b2)
    }

    /// return receiveQuoteToken
    pub fn r_above_sell_base_token(
        &self,
        amount: FixedU256,
        base_balance: FixedU256,
        target_base_amount: FixedU256,
    ) -> Option<FixedU256> {
        // here we don't require B1 <= targetBaseAmount
        // Because it is limited at upper function

        let b1 = base_balance.checked_add(amount)?;

        self.r_above_integrate(target_base_amount, b1, base_balance)
    }

    /// return payBaseToken
    pub fn r_above_back_to_one(&self) -> Option<FixedU256> {
        let spare_quote = self
            .quote_balance
            .checked_sub(self.target_quote_token_amount)?;
        let price = self.oracle;
        let fair_amount = spare_quote.checked_div_floor(price)?;
        let new_target_base =
            solve_quadratic_function_for_target(self.base_balance, self.k, fair_amount)?;

        new_target_base.checked_sub(self.base_balance)
    }

    // helper functions
    // ============ Helper functions ============

    /// return baseTarget, quoteTarget
    pub fn get_expected_target(&self) -> Option<(FixedU256, FixedU256)> {
        let q = self.quote_balance;
        let b = self.base_balance;

        if self.r_status == RStatus::One {
            Some((
                self.target_base_token_amount,
                self.target_quote_token_amount,
            ))
        } else if self.r_status == RStatus::BelowOne {
            let pay_quote_token = self.r_below_back_to_one()?;

            Some((
                self.target_base_token_amount,
                q.checked_add(pay_quote_token)?,
            ))
        } else {
            let pay_base_token = self.r_above_back_to_one()?;

            Some((
                b.checked_add(pay_base_token)?,
                self.target_quote_token_amount,
            ))
        }
    }

    /// return midPrice
    fn _get_mid_price(&self) -> Option<FixedU256> {
        let (base_target, quote_target) = self.get_expected_target()?;

        if self.r_status == RStatus::BelowOne {
            let mut r = quote_target
                .checked_mul_floor(quote_target)?
                .checked_div_floor(self.quote_balance.checked_mul_floor(self.quote_balance)?)?;

            r = FixedU256::one()
                .checked_sub(self.k)?
                .checked_add(self.k.checked_mul_floor(r)?)?;

            self.oracle.checked_div_floor(r)
        } else {
            let mut r = base_target
                .checked_mul_floor(base_target)?
                .checked_div_floor(self.base_balance.checked_mul_floor(self.base_balance)?)?;

            r = FixedU256::one()
                .checked_sub(self.k)?
                .checked_add(self.k.checked_mul_floor(r)?)?;

            self.oracle.checked_mul_floor(r)
        }
    }

    fn r_above_integrate(&self, v0: FixedU256, v1: FixedU256, v2: FixedU256) -> Option<FixedU256> {
        let i = self.oracle;

        general_integrate(v0, v1, v2, i, self.k)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bn::FixedU256,
        v1curve::{RStatus, V1curve},
    };

    #[test]
    fn basic() {
        let k: FixedU256 = FixedU256::one()
            .checked_mul_floor(FixedU256::new(5.into()))
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();
        let mut r_status = RStatus::One;
        let oracle: FixedU256 = FixedU256::new_from_int(100.into(), 18).unwrap();
        let base_balance: FixedU256 = FixedU256::new_from_int(1000.into(), 18).unwrap();
        let quote_balance: FixedU256 = FixedU256::new_from_int(2000.into(), 18).unwrap();
        let target_base_token_amount: FixedU256 = FixedU256::new_from_int(500.into(), 18).unwrap();
        let target_quote_token_amount: FixedU256 =
            FixedU256::new_from_int(1000.into(), 18).unwrap();

        let mut v1_curve = V1curve::new(
            k,
            r_status,
            oracle,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
        );

        let amount: FixedU256 = FixedU256::new_from_int(200.into(), 18).unwrap();

        // ================== R = 1 cases ==================

        assert_eq!(
            v1_curve
                .r_one_sell_base_token(amount, target_quote_token_amount)
                .unwrap(),
            FixedU256::new_from_int(975.into(), 18).unwrap()
        );

        assert_eq!(
            v1_curve
                .r_one_buy_base_token(amount, target_base_token_amount)
                .unwrap(),
            FixedU256::new_from_int(30000.into(), 18).unwrap()
        );

        // ============ R < 1 cases ============
        r_status = RStatus::BelowOne;
        v1_curve = V1curve::new(
            k,
            r_status,
            oracle,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
        );

        assert_eq!(
            v1_curve
                .r_below_sell_base_token(amount, quote_balance, target_quote_token_amount)
                .unwrap(),
            FixedU256::new_from_int(1974.into(), 18).unwrap()
        );

        assert_eq!(
            v1_curve
                .r_below_buy_base_token(amount, quote_balance, target_quote_token_amount)
                .unwrap(),
            FixedU256::new_from_int(39524.into(), 18).unwrap()
        );

        assert_eq!(
            v1_curve.r_below_back_to_one().unwrap(),
            FixedU256::new_from_int(14000.into(), 18).unwrap()
        );

        // ============ R > 1 cases ============
        r_status = RStatus::AboveOne;
        v1_curve = V1curve::new(
            k,
            r_status,
            oracle,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
        );

        assert_eq!(
            v1_curve
                .r_above_buy_base_token(amount, base_balance, target_base_token_amount)
                .unwrap(),
            FixedU256::new_from_int(13125.into(), 18).unwrap()
        );

        assert_eq!(
            v1_curve
                .r_above_sell_base_token(amount, base_balance, target_base_token_amount)
                .unwrap(),
            FixedU256::new_from_int(12080.into(), 18).unwrap()
        );

        let value = FixedU256::new(995049300.into())
            .checked_mul_floor(FixedU256::new(100000.into()))
            .unwrap()
            .checked_mul_floor(FixedU256::new(100000.into()))
            .unwrap();
        assert_eq!(
            v1_curve.r_above_back_to_one().unwrap(),
            FixedU256::new_from_fixed(value.into_u256_ceil(), 18)
        );

        // ============ Helper functions ============

        r_status = RStatus::BelowOne;
        v1_curve = V1curve::new(
            k,
            r_status,
            oracle,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
        );

        assert_eq!(
            v1_curve.get_expected_target().unwrap(),
            (
                FixedU256::new_from_int(500.into(), 18).unwrap(),
                FixedU256::new_from_int(16000.into(), 18).unwrap()
            )
        );

        assert_eq!(
            v1_curve._get_mid_price().unwrap(),
            FixedU256::new_from_int(3.into(), 18).unwrap()
        );

        assert_eq!(
            v1_curve
                .r_above_integrate(
                    target_quote_token_amount,
                    target_quote_token_amount,
                    target_quote_token_amount.checked_sub(amount).unwrap()
                )
                .unwrap(),
            FixedU256::new_from_int(30000.into(), 18).unwrap()
        );
    }
}
