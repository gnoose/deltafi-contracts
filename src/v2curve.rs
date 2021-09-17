//! Implement pricing of PMM
use solana_program::program_error::ProgramError;

use crate::{
    bn::FixedU256,
    math2::{
        general_integrate, solve_quadratic_function_for_target, solve_quadratic_function_for_trade,
    },
};

/// RStatus enum
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
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
        RState::One
    }
}

/// PMMState struct
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PMMState {
    /// i
    pub i: FixedU256,

    /// k
    pub k: FixedU256,

    /// b
    pub b: FixedU256,

    /// q
    pub q: FixedU256,

    /// b_0
    pub b_0: FixedU256,

    /// q_0
    pub q_0: FixedU256,

    /// r
    pub r: RState,
}

impl PMMState {
    /// initialize PMMState
    pub fn new(
        i: FixedU256,
        k: FixedU256,
        b: FixedU256,
        q: FixedU256,
        b_0: FixedU256,
        q_0: FixedU256,
        r: RState,
    ) -> Self {
        Self {
            i,
            k,
            b,
            q,
            b_0,
            q_0,
            r,
        }
    }
}

// ================== buy & sell ===================

/// return receive_quote_amount and r_status
pub fn sell_base_token(
    state: PMMState,
    pay_base_amount: FixedU256,
) -> Result<(FixedU256, RState), ProgramError> {
    let mut receive_quote_amount;
    let new_r;
    if state.r == RState::One {
        // case 1: R=1
        // R falls below one

        receive_quote_amount = r_one_sell_base_token(state, pay_base_amount)?;
        new_r = RState::BelowOne;
    } else if state.r == RState::AboveOne {
        let back_to_one_pay_base = state.b_0.checked_sub(state.b)?;
        let back_to_one_receive_quote = state.q.checked_sub(state.q_0)?;

        // case 2: R>1
        // complex case, R status depends on trading amount

        if pay_base_amount.into_u256_ceil() < back_to_one_pay_base.into_u256_ceil() {
            // case 2.1: R status do not change
            receive_quote_amount = r_above_sell_base_token(state, pay_base_amount)?;
            new_r = RState::AboveOne;

            if receive_quote_amount.into_u256_ceil() > back_to_one_receive_quote.into_u256_ceil() {
                // [Important corner case!] may enter this branch when some precision problem happens. And consequently contribute to negative spare quote amount
                // to make sure spare quote>=0, mannually set receiveQuote=backToOneReceiveQuote

                receive_quote_amount = back_to_one_receive_quote;
            }
        } else if pay_base_amount.into_u256_ceil() == back_to_one_pay_base.into_u256_ceil() {
            // case 2.2: R status changes to ONE
            receive_quote_amount = back_to_one_receive_quote;
            new_r = RState::One;
        } else {
            // case 2.3: R status changes to BELOW_ONE
            receive_quote_amount = back_to_one_receive_quote.checked_add(r_one_sell_base_token(
                state,
                pay_base_amount.checked_sub(back_to_one_pay_base)?,
            )?)?;
            new_r = RState::BelowOne;
        }
    } else {
        // state.R == RState.BELOW_ONE
        // case 3: R<1
        receive_quote_amount = r_below_sell_base_token(state, pay_base_amount)?;
        new_r = RState::BelowOne;
    }

    Ok((receive_quote_amount, new_r))
}

/// return receive_base_amount and r_status
pub fn sell_quote_token(
    state: PMMState,
    pay_quote_amount: FixedU256,
) -> Result<(FixedU256, RState), ProgramError> {
    let mut receive_base_amount;
    let new_r;
    if state.r == RState::One {
        receive_base_amount = r_one_sell_quote_token(state, pay_quote_amount)?;
        new_r = RState::AboveOne;
    } else if state.r == RState::AboveOne {
        receive_base_amount = r_above_sell_quote_token(state, pay_quote_amount)?;
        new_r = RState::AboveOne;
    } else {
        let back_to_one_pay_quote = state.q_0.checked_sub(state.q)?;
        let back_to_one_receive_base = state.b.checked_sub(state.b_0)?;

        if pay_quote_amount.into_u256_ceil() < back_to_one_pay_quote.into_u256_ceil() {
            receive_base_amount = r_below_sell_quote_token(state, pay_quote_amount)?;
            new_r = RState::BelowOne;

            if receive_base_amount.into_u256_ceil() > back_to_one_receive_base.into_u256_ceil() {
                receive_base_amount = back_to_one_receive_base;
            }
        } else if pay_quote_amount.into_u256_ceil() == back_to_one_pay_quote.into_u256_ceil() {
            receive_base_amount = back_to_one_receive_base;
            new_r = RState::One;
        } else {
            receive_base_amount = back_to_one_receive_base.checked_add(r_one_sell_quote_token(
                state,
                pay_quote_amount.checked_sub(back_to_one_pay_quote)?,
            )?)?;
            new_r = RState::AboveOne;
        }
    }

    Ok((receive_base_amount, new_r))
}

// ============ R = 1 cases ============

/// receiveQuoteToken
pub fn r_one_sell_base_token(
    state: PMMState,
    pay_base_amount: FixedU256,
) -> Result<FixedU256, ProgramError> {
    // in theory Q2 <= targetQuoteTokenAmount
    // however when amount is close to 0, precision problems may cause Q2 > targetQuoteTokenAmount
    solve_quadratic_function_for_trade(state.q_0, state.q_0, pay_base_amount, state.i, state.k)
}

/// receiveBaseToken
pub fn r_one_sell_quote_token(
    state: PMMState,
    pay_quote_amount: FixedU256,
) -> Result<FixedU256, ProgramError> {
    solve_quadratic_function_for_trade(
        state.b_0,
        state.b_0,
        pay_quote_amount,
        FixedU256::reciprocal_floor(state.i)?,
        state.k,
    )
}

// ============ R < 1 cases ============

/// receiveBaseToken
pub fn r_below_sell_quote_token(
    state: PMMState,
    pay_quote_amount: FixedU256,
) -> Result<FixedU256, ProgramError> {
    general_integrate(
        state.q_0,
        state.q.checked_add(pay_quote_amount)?,
        state.q,
        FixedU256::reciprocal_floor(state.i)?,
        state.k,
    )
}

/// receiveQuoteToken
pub fn r_below_sell_base_token(
    state: PMMState,
    pay_base_amount: FixedU256,
) -> Result<FixedU256, ProgramError> {
    solve_quadratic_function_for_trade(state.q_0, state.q, pay_base_amount, state.i, state.k)
}

// ============ R > 1 cases ============

/// receiveQuoteToken
pub fn r_above_sell_base_token(
    state: PMMState,
    pay_base_amount: FixedU256,
) -> Result<FixedU256, ProgramError> {
    general_integrate(
        state.b_0,
        state.b.checked_add(pay_base_amount)?,
        state.b,
        state.i,
        state.k,
    )
}

/// receiveBaseToken
pub fn r_above_sell_quote_token(
    state: PMMState,
    pay_quote_amount: FixedU256,
) -> Result<FixedU256, ProgramError> {
    solve_quadratic_function_for_trade(
        state.b_0,
        state.b,
        pay_quote_amount,
        FixedU256::reciprocal_floor(state.i)?,
        state.k,
    )
}

// ============ Helper functions ============

/// adjust target value
pub fn adjusted_target(state: PMMState) -> Result<PMMState, ProgramError> {
    if state.r == RState::BelowOne {
        let new_q_0 = solve_quadratic_function_for_target(
            state.q,
            state.b.checked_sub(state.b_0)?,
            state.i,
            state.k,
        )?;
        Ok(PMMState::new(
            state.i, state.k, state.b, state.q, state.b_0, new_q_0, state.r,
        ))
    } else if state.r == RState::AboveOne {
        let new_b_0 = solve_quadratic_function_for_target(
            state.b,
            state.q.checked_sub(state.q_0)?,
            FixedU256::reciprocal_floor(state.i)?,
            state.k,
        )?;
        Ok(PMMState::new(
            state.i, state.k, state.b, state.q, new_b_0, state.q_0, state.r,
        ))
    } else {
        Ok(state)
    }
}

/// get mid price
pub fn get_mid_price(state: PMMState) -> Result<FixedU256, ProgramError> {
    if state.r == RState::BelowOne {
        let mut r = state
            .q_0
            .checked_mul_floor(state.q_0)?
            .checked_div_floor(state.q)?
            .checked_div_floor(state.q)?;
        r = FixedU256::one()
            .checked_sub(state.k)?
            .checked_add(state.k.checked_mul_floor(r)?)?;

        state.i.checked_div_floor(r)
    } else {
        let mut r = state
            .b_0
            .checked_mul_floor(state.b_0)?
            .checked_div_floor(state.b)?
            .checked_div_floor(state.b)?;
        r = FixedU256::one()
            .checked_sub(state.k)?
            .checked_add(state.k.checked_mul_floor(r)?)?;

        state.i.checked_mul_floor(r)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bn::FixedU256,
        v2curve::{
            adjusted_target, get_mid_price, r_above_sell_base_token, r_above_sell_quote_token,
            r_below_sell_base_token, r_below_sell_quote_token, r_one_sell_base_token,
            r_one_sell_quote_token, PMMState, RState,
        },
    };

    #[test]
    fn basic() {
        let k: FixedU256 = FixedU256::one()
            .checked_mul_floor(FixedU256::new(5.into()))
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();
        let mut r = RState::One;
        let i: FixedU256 = FixedU256::new_from_int(100.into(), 18).unwrap();
        let base_balance: FixedU256 = FixedU256::new_from_int(1000.into(), 18).unwrap();
        let quote_balance: FixedU256 = FixedU256::new_from_int(2000.into(), 18).unwrap();
        let target_base_token_amount: FixedU256 = FixedU256::new_from_int(500.into(), 18).unwrap();
        let target_quote_token_amount: FixedU256 =
            FixedU256::new_from_int(1000.into(), 18).unwrap();

        let mut state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
            r,
        );

        let amount: FixedU256 = FixedU256::new_from_int(200.into(), 18).unwrap();
        // ================== R = 1 cases ==================

        assert_eq!(
            r_one_sell_base_token(state, amount).unwrap(),
            FixedU256::new_from_int(952.into(), 18).unwrap()
        );

        assert_eq!(
            r_one_sell_quote_token(state, amount).unwrap(),
            FixedU256::new_from_int(1.into(), 18).unwrap()
        );

        // ============ R < 1 cases ============
        r = RState::BelowOne;
        state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
            r,
        );

        assert_eq!(
            r_below_sell_base_token(state, amount).unwrap(),
            FixedU256::new_from_int(1951.into(), 18).unwrap()
        );

        let value = FixedU256::new_from_int(1227.into(), 18)
            .unwrap()
            .checked_div_floor(FixedU256::new(1000.into()))
            .unwrap();
        assert_eq!(r_below_sell_quote_token(state, amount).unwrap(), value);

        // ============ R > 1 cases ============
        r = RState::AboveOne;
        state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
            r,
        );

        assert_eq!(
            r_above_sell_base_token(state, amount).unwrap(),
            FixedU256::new_from_int(12080.into(), 18).unwrap()
        );

        assert_eq!(
            r_above_sell_quote_token(state, amount).unwrap(),
            FixedU256::new_from_int(7.into(), 18).unwrap()
        );

        // ============ Helper functions ============
        r = RState::One;
        state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            target_base_token_amount,
            target_quote_token_amount,
            r,
        );

        assert_eq!(adjusted_target(state).unwrap(), state);

        let value = FixedU256::new_from_int(625.into(), 18)
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();
        assert_eq!(get_mid_price(state).unwrap(), value);
    }
}
