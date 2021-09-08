//! Implement pricing of PMM
use crate::{
    bn::U256,
    math::{
        div_floor, general_integrate, mul_ceil, solve_quadratic_function_for_target,
        solve_quadratic_function_for_trade,
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
    pub _k_: U256,

    /// r status
    pub _r_status_: RStatus,

    /// oracle price
    pub _oracle_: U256,

    /// base token balance
    pub _base_balance_: U256,

    /// quote token balance
    pub _quote_balance_: U256,

    /// target base token amount
    pub _target_base_token_amount_: U256,

    /// target quote token amount
    pub _target_quote_token_amount_: U256,
}

impl V1curve {
    /// initialize function for V1curve
    pub fn new(
        _k_: U256,
        _r_status_: RStatus,
        _oracle_: U256,
        _base_balance_: U256,
        _quote_balance_: U256,
        _target_base_token_amount_: U256,
        _target_quote_token_amount_: U256,
    ) -> Self {
        Self {
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        }
    }
    // ================== R = 1 cases ==================

    /// return receiveQuoteToken
    pub fn _r_one_sell_base_token(
        &self,
        amount: U256,
        target_quote_token_amount: U256,
    ) -> Option<U256> {
        let i = self._oracle_;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_token_amount,
            target_quote_token_amount,
            i.checked_mul(amount)?,
            false,
            self._k_,
        )?;

        // in theory Q2 <= target_quote_token_amount
        // however when amount is close to 0, precision problems may cause Q2 > target_quote_token_amount

        target_quote_token_amount.checked_sub(q2)
    }

    /// return payQuoteToken
    pub fn _r_one_buy_base_token(
        &self,
        amount: U256,
        target_base_token_amount: U256,
    ) -> Option<U256> {
        let b2 = target_base_token_amount.checked_sub(amount)?;

        self._r_above_integrate(target_base_token_amount, target_base_token_amount, b2)
    }

    // ============ R < 1 cases ============

    /// return receieQuoteToken
    pub fn _r_below_sell_base_token(
        &self,
        amount: U256,
        quote_balance: U256,
        target_quote_amount: U256,
    ) -> Option<U256> {
        let i = self._oracle_;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            i.checked_mul(amount)?,
            false,
            self._k_,
        )?;

        quote_balance.checked_sub(q2)
    }

    /// return payQuoteToken
    pub fn _r_below_buy_base_token(
        &self,
        amount: U256,
        quote_balance: U256,
        target_quote_amount: U256,
    ) -> Option<U256> {
        // Here we don't require amount less than some value
        // Because it is limited at upper function

        let i = self._oracle_;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            mul_ceil(i, amount)?,
            true,
            self._k_,
        )?;

        q2.checked_sub(quote_balance)
    }

    /// return payQuoteToken
    pub fn _r_below_back_to_one(&self) -> Option<U256> {
        // important: carefully design the system to make sure spareBase always greater than or equal to 0

        let spare_base = self
            ._base_balance_
            .checked_sub(self._target_base_token_amount_)?;
        let price = self._oracle_;
        let fair_amount = spare_base.checked_mul(price)?;
        let new_target_quote =
            solve_quadratic_function_for_target(self._quote_balance_, self._k_, fair_amount)?;

        new_target_quote.checked_sub(self._quote_balance_)
    }

    // ============ R > 1 cases ============

    /// return payQuoteToken
    pub fn _r_above_buy_base_token(
        &self,
        amount: U256,
        base_balance: U256,
        target_base_amount: U256,
    ) -> Option<U256> {
        //require(amount < baseBalance, "DODO_BASE_BALANCE_NOT_ENOUGH");

        let b2 = base_balance.checked_sub(amount)?;

        self._r_above_integrate(target_base_amount, base_balance, b2)
    }

    /// return receiveQuoteToken
    pub fn _r_above_sell_base_token(
        &self,
        amount: U256,
        base_balance: U256,
        target_base_amount: U256,
    ) -> Option<U256> {
        // here we don't require B1 <= targetBaseAmount
        // Because it is limited at upper function

        let b1 = base_balance.checked_add(amount)?;

        self._r_above_integrate(target_base_amount, b1, base_balance)
    }

    /// return payBaseToken
    pub fn _r_above_back_to_one(&self) -> Option<U256> {
        let spare_quote = self
            ._quote_balance_
            .checked_sub(self._target_quote_token_amount_)?;
        let price = self._oracle_;
        let fair_amount = div_floor(spare_quote, price)?;
        let new_target_base =
            solve_quadratic_function_for_target(self._base_balance_, self._k_, fair_amount)?;

        new_target_base.checked_sub(self._base_balance_)
    }

    // helper functions
    // ============ Helper functions ============

    /// return baseTarget, quoteTarget
    fn get_expected_target(&self) -> Option<(U256, U256)> {
        let q = self._quote_balance_;
        let b = self._base_balance_;

        if self._r_status_ == RStatus::One {
            Some((
                self._target_base_token_amount_,
                self._target_quote_token_amount_,
            ))
        } else if self._r_status_ == RStatus::BelowOne {
            let pay_quote_token = self._r_below_back_to_one()?;

            Some((
                self._target_base_token_amount_,
                q.checked_add(pay_quote_token)?,
            ))
        } else {
            let pay_base_token = self._r_above_back_to_one()?;

            Some((
                b.checked_add(pay_base_token)?,
                self._target_quote_token_amount_,
            ))
        }
    }

    /// return midPrice
    fn get_mid_price(&self) -> Option<U256> {
        let (base_target, quote_target) = self.get_expected_target()?;

        if self._r_status_ == RStatus::BelowOne {
            let mut r = div_floor(
                quote_target.checked_mul(quote_target)?,
                self._quote_balance_.checked_mul(self._quote_balance_)?,
            )?;
            r = U256::one()
                .checked_sub(self._k_)?
                .checked_add(self._k_.checked_mul(r)?)?;

            div_floor(self._oracle_, r)
        } else {
            let mut r = div_floor(
                base_target.checked_mul(base_target)?,
                self._base_balance_.checked_mul(self._base_balance_)?,
            )?;
            r = U256::one()
                .checked_sub(self._k_)?
                .checked_add(self._k_.checked_mul(r)?)?;

            self._oracle_.checked_mul(r)
        }
    }

    fn _r_above_integrate(&self, b0: U256, b1: U256, b2: U256) -> Option<U256> {
        let i = self._oracle_;

        general_integrate(b0, b1, b2, i, self._k_)
    }
}

#[cfg(test)]
mod test {

    use rand::Rng;

    use super::*;
    use crate::{
        bn::U256,
        math::{solve_quadratic_function_for_target, solve_quadratic_function_for_trade},
        v1curve::{RStatus, V1curve},
    };

    /// const variable definitions
    pub const ZERO_V: u64 = 0 as u64;
    pub const ONE_V: u64 = 1 as u64;
    pub const TWO_V: u64 = 2 as u64;
    pub const FOURTH_V: u64 = u64::MAX / 4;
    pub const HALF_V: u64 = u64::MAX / 2;
    pub const MAX_V: u64 = u64::MAX;

    #[test]
    fn test_r_one_sell_base_token() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let amount: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let target_quote_token_amount: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let q2 = solve_quadratic_function_for_trade(
            _target_quote_token_amount_,
            _target_quote_token_amount_,
            _oracle_.checked_mul(amount).unwrap(),
            false,
            _k_,
        )
        .unwrap();

        let expected = target_quote_token_amount.checked_sub(q2).unwrap();
        assert_eq!(
            v1_curve
                ._r_one_sell_base_token(amount, target_quote_token_amount)
                .unwrap(),
            expected
        )
    }

    #[test]
    fn test_r_one_buy_base_token() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let amount: U256 = rand::thread_rng().gen_range(ONE_V, FOURTH_V).into();
        let target_base_token_amount: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let b2 = target_base_token_amount.checked_sub(amount).unwrap();

        let expected = v1_curve
            ._r_above_integrate(target_base_token_amount, target_base_token_amount, b2)
            .unwrap();

        assert_eq!(
            v1_curve
                ._r_one_buy_base_token(amount, target_base_token_amount)
                .unwrap(),
            expected
        )
    }

    #[test]
    fn test_r_below_sell_base_token() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let amount: U256 = rand::thread_rng().gen_range(ONE_V, FOURTH_V).into();
        let quote_balance: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let target_quote_amount: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            _oracle_.checked_mul(amount).unwrap(),
            false,
            _k_,
        )
        .unwrap();

        let expected = quote_balance.checked_sub(q2).unwrap();

        assert_eq!(
            v1_curve
                ._r_below_sell_base_token(amount, quote_balance, target_quote_amount)
                .unwrap(),
            expected
        )
    }

    #[test]
    fn test_r_below_buy_base_token() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let amount: U256 = rand::thread_rng().gen_range(ONE_V, FOURTH_V).into();
        let quote_balance: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let target_quote_amount: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            mul_ceil(_oracle_, amount).unwrap(),
            true,
            _k_,
        )
        .unwrap();

        let expected = q2.checked_sub(quote_balance).unwrap();

        assert_eq!(
            v1_curve
                ._r_below_buy_base_token(amount, quote_balance, target_quote_amount)
                .unwrap(),
            expected
        )
    }

    #[test]
    fn test_r_below_back_to_one() {
        let _k_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let spare_base = _base_balance_
            .checked_sub(_target_base_token_amount_)
            .unwrap();
        let fair_amount = spare_base.checked_mul(_oracle_).unwrap();
        let new_target_quote =
            solve_quadratic_function_for_target(_quote_balance_, _k_, fair_amount).unwrap();

        let expected = new_target_quote.checked_sub(_quote_balance_).unwrap();

        assert_eq!(v1_curve._r_below_back_to_one().unwrap(), expected)
    }

    #[test]
    fn test_r_above_buy_base_token() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();

        let amount: U256 = rand::thread_rng().gen_range(ONE_V, FOURTH_V).into();
        let base_balance: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let target_base_amount: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let b2 = base_balance.checked_sub(amount).unwrap();

        let expected = v1_curve
            ._r_above_integrate(target_base_amount, base_balance, b2)
            .unwrap();

        assert_eq!(
            v1_curve
                ._r_above_buy_base_token(amount, base_balance, target_base_amount)
                .unwrap(),
            expected
        )
    }

    #[test]
    fn test_r_above_sell_base_token() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();

        let amount: U256 = rand::thread_rng().gen_range(ONE_V, FOURTH_V).into();
        let base_balance: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let target_base_amount: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let b1 = base_balance.checked_add(amount).unwrap();

        let expected = v1_curve
            ._r_above_integrate(target_base_amount, b1, base_balance)
            .unwrap();

        assert_eq!(
            v1_curve
                ._r_above_sell_base_token(amount, base_balance, target_base_amount)
                .unwrap(),
            expected
        )
    }

    #[test]
    fn test_r_above_back_to_one() {
        let _k_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(HALF_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, HALF_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let spare_quote = _quote_balance_
            .checked_sub(_target_quote_token_amount_)
            .unwrap();
        let fair_amount = div_floor(spare_quote, _oracle_).unwrap();
        let new_target_base =
            solve_quadratic_function_for_target(_base_balance_, _k_, fair_amount).unwrap();

        let expected = new_target_base.checked_sub(_base_balance_).unwrap();

        assert_eq!(v1_curve._r_above_back_to_one().unwrap(), expected)
    }

    #[test]
    fn test_get_expected_target() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );
        let expected;
        if _r_status_ == RStatus::One {
            expected = (_target_base_token_amount_, _target_quote_token_amount_);
        } else if _r_status_ == RStatus::BelowOne {
            let pay_quote_token = v1_curve._r_below_back_to_one().unwrap();

            expected = (
                _target_base_token_amount_,
                _quote_balance_.checked_add(pay_quote_token).unwrap(),
            );
        } else {
            let pay_base_token = v1_curve._r_above_back_to_one().unwrap();

            expected = (
                _base_balance_.checked_add(pay_base_token).unwrap(),
                _target_quote_token_amount_,
            );
        }

        assert_eq!(v1_curve.get_expected_target().unwrap(), expected)
    }

    #[test]
    fn test_get_mid_price() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let (base_target, quote_target) = v1_curve.get_expected_target().unwrap();

        let expected;
        if _r_status_ == RStatus::BelowOne {
            let mut r = div_floor(
                quote_target.checked_mul(quote_target).unwrap(),
                _quote_balance_.checked_mul(_quote_balance_).unwrap(),
            )
            .unwrap();
            r = U256::one()
                .checked_sub(_k_)
                .unwrap()
                .checked_add(_k_.checked_mul(r).unwrap())
                .unwrap();

            expected = div_floor(_oracle_, r).unwrap();
        } else {
            let mut r = div_floor(
                base_target.checked_mul(base_target).unwrap(),
                _base_balance_.checked_mul(_base_balance_).unwrap(),
            )
            .unwrap();
            r = U256::one()
                .checked_sub(_k_)
                .unwrap()
                .checked_add(_k_.checked_mul(r).unwrap())
                .unwrap();

            expected = _oracle_.checked_mul(r).unwrap();
        }

        assert_eq!(v1_curve.get_mid_price().unwrap(), expected)
    }

    #[test]
    fn test_r_above_integrate() {
        let _k_: U256 = rand::thread_rng().gen_range(ZERO_V, ONE_V).into();
        let _r_status_ = RStatus::One;
        let _oracle_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _base_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _quote_balance_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_base_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();
        let _target_quote_token_amount_: U256 = rand::thread_rng().gen_range(ONE_V, MAX_V).into();

        let b0: U256 = rand::thread_rng().gen_range(TWO_V, FOURTH_V).into();
        let b1: U256 = b0.checked_mul(3.into()).unwrap();
        let b2: U256 = b0.checked_mul(2.into()).unwrap();

        let v1_curve = V1curve::new(
            _k_,
            _r_status_,
            _oracle_,
            _base_balance_,
            _quote_balance_,
            _target_base_token_amount_,
            _target_quote_token_amount_,
        );

        let expected = general_integrate(b0, b1, b2, _oracle_, _k_).unwrap();

        assert_eq!(v1_curve._r_above_integrate(b0, b1, b2).unwrap(), expected)
    }
}
