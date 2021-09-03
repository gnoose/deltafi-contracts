//! Implement pricing of PMM
use crate::bn::U256;
use crate::math::{
    div_floor,
    mul_ceil,
    general_integrate,
    solve_quadratic_function_for_trade,
    solve_quadratic_function_for_target
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
    BelowOne
}

impl Default for RStatus {
    fn default() -> Self { RStatus::One }
}

/// Pricing struct
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pricing {
    /// slope variable
    pub _k_: U256,

    /// r status
    pub _r_status_: RStatus,

    /// oracle price
    pub _oracle_: U256,

    /// base token balance
    pub  _base_balance_: U256,

    /// quote token balance
    pub _quote_balance_: U256,

    /// target base token amount
    pub _target_base_token_amount_: U256,

    /// target quote token amount
    pub _target_quote_token_amount_: U256,

}

impl Pricing {
    /// initialize function for Pricing
    pub fn new(
        _k_: U256,
        _r_status_: RStatus,
        _oracle_: U256,
        _base_balance_: U256,
        _quote_balance_: U256,
        _target_base_token_amount_: U256,
        _target_quote_token_amount_: U256
    ) -> Self {
        Self {
            _k_: _k_,
            _r_status_: _r_status_,
            _oracle_: _oracle_,
            _base_balance_: _base_balance_,
            _quote_balance_: _quote_balance_,
            _target_base_token_amount_: _target_base_token_amount_,
            _target_quote_token_amount_: _target_quote_token_amount_
        }
    }
    // ================== R = 1 cases ==================

    /// return receiveQuoteToken
    pub fn _r_one_sell_base_token(
        &self,
        amount: U256,
        target_quote_token_amount: U256
    ) -> U256 {
        let i = self._oracle_;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_token_amount,
            target_quote_token_amount,
            i * amount,
            false,
            self._k_
        );

        // in theory Q2 <= target_quote_token_amount
        // however when amount is close to 0, precision problems may cause Q2 > target_quote_token_amount

        target_quote_token_amount - q2
    }

    /// return payQuoteToken
    pub fn _r_one_buy_base_token(
        &self,
        amount: U256,
        target_base_token_amount: U256
    ) -> U256 {
        let b2 = target_base_token_amount - amount;

        self._r_above_integrate(target_base_token_amount, target_base_token_amount, b2)
    }

    // ============ R < 1 cases ============

    /// return receieQuoteToken
    pub fn _r_below_sell_base_token(
        &self,
        amount: U256,
        quote_balance: U256,
        target_quote_amount: U256
    ) -> U256 {
        let i = self._oracle_;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            i * amount,
            false,
            self._k_
        );

        quote_balance - q2
    }

    /// return payQuoteToken
    pub fn _r_below_by_base_token(
        &self,
        amount: U256,
        quote_balance: U256,
        target_quote_amount: U256
    ) -> U256 {
        // Here we don't require amount less than some value
        // Because it is limited at upper function

        let i = self._oracle_;
        let q2 = solve_quadratic_function_for_trade(
            target_quote_amount,
            quote_balance,
            mul_ceil(i, amount),
            true,
            self._k_
        );

        q2 - quote_balance
    }

    /// return payQuoteToken
    pub fn _r_below_back_to_one(&self) -> U256 {
        // important: carefully design the system to make sure spareBase always greater than or equal to 0

        let spare_base = self._base_balance_ - self._target_base_token_amount_;
        let price = self._oracle_;
        let fair_amount = spare_base * price;
        let new_target_quote = solve_quadratic_function_for_target(
            self._quote_balance_,
            self._k_,
            fair_amount
        );

        new_target_quote - self._quote_balance_
    }

    // ============ R > 1 cases ============

    /// return payQuoteToken
    pub fn _r_above_buy_base_token(
        &self,
        amount: U256,
        base_balance: U256,
        target_base_amount: U256
    ) -> U256 {
        //require(amount < baseBalance, "DODO_BASE_BALANCE_NOT_ENOUGH");

        let b2 = base_balance - amount;

        self._r_above_integrate(target_base_amount, base_balance, b2)
    }

    /// return receiveQuoteToken
    pub fn _r_above_sell_base_token(
        &self,
        amount: U256,
        base_balance: U256,
        target_base_amount: U256
    ) -> U256 {
        // here we don't require B1 <= targetBaseAmount
        // Because it is limited at upper function

        let b1 = base_balance + amount;

        self._r_above_integrate(target_base_amount, b1, base_balance)
    }

    /// return payBaseToken
    pub fn _r_above_back_to_one(&self) -> U256 {
        let spare_quote = self._quote_balance_ - self._target_quote_token_amount_;
        let price = self._oracle_;
        let fair_amount = div_floor(spare_quote, price);
        let new_target_base = solve_quadratic_function_for_target(
            self._base_balance_,
            self._k_,
            fair_amount
        );

        new_target_base - self._base_balance_
    }

    // helper functions
    // ============ Helper functions ============

    /// return baseTarget, quoteTarget
    fn get_expected_target(&self) -> (U256, U256) {
        let q = self._quote_balance_;
        let b = self._base_balance_;

        if self._r_status_ == RStatus::One {
            (self._target_base_token_amount_, self._target_quote_token_amount_)
        } else if self._r_status_ == RStatus::BelowOne {
            let pay_quote_token = self._r_below_back_to_one();

            (self._target_base_token_amount_, q + pay_quote_token)
        } else {
            let pay_base_token = self._r_above_back_to_one();

            (b + pay_base_token, self._target_quote_token_amount_)
        }
    }

    /// return midPrice
    fn get_mid_price(&self) -> U256 {
        let (base_target, quote_target) = self.get_expected_target();

        if self._r_status_ == RStatus::BelowOne {
            let mut r = div_floor(quote_target * quote_target, self._quote_balance_ * self._quote_balance_);
            r = U256::from(1) - self._k_ + self._k_ * r;

            div_floor(self._oracle_, r)
        } else {
            let mut r = div_floor(base_target * base_target, self._base_balance_ * self._base_balance_);
            r = U256::from(1) - self._k_ + self._k_ * r;

            self._oracle_ * r
        }
    }

    fn _r_above_integrate(
        &self,
        b0: U256,
        b1: U256,
        b2: U256
    ) -> U256 {
        let i = self._oracle_;
        general_integrate(b0, b1, b2, i, self._k_)
    }

}

#[cfg(test)]
mod test {

    #[test]
    fn test_r_one_sell_base_token() {

    }

    #[test]
    fn test_r_one_buy_base_token() {

    }

    #[test]
    fn test_r_below_sell_base_token() {

    }

    #[test]
    fn test_r_below_by_base_token() {

    }

    #[test]
    fn test_r_below_back_to_one() {

    }

    #[test]
    fn test_r_above_buy_base_token() {

    }

    #[test]
    fn test_r_above_sell_base_token() {

    }

    #[test]
    fn test_r_above_back_to_one() {

    }

    #[test]
    fn test_get_expected_target() {

    }

    #[test]
    fn test_get_mid_price() {

    }

    #[test]
    fn test_r_above_integrate() {

    }



}