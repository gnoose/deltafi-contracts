//! pricing for proactive market maker
use std::mem::size_of;

use solana_program::{entrypoint::ProgramResult, program_error::ProgramError};

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

impl RState {
    /// Unpacks a byte buffer into a [RState](enum.RState.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, _rest) = input.split_first().ok_or(ProgramError::InvalidArgument)?;
        Ok(match tag {
            110 => Self::One,
            111 => Self::AboveOne,
            112 => Self::BelowOne,
            _ => Self::One,
        })
    }

    /// Packs a [RState](enum.RState.html) into a byte buffer.
    pub fn pack(&self) -> [u8; 1] {
        let mut buf: Vec<u8> = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::One => buf.push(110),
            Self::AboveOne => buf.push(111),
            Self::BelowOne => buf.push(112),
        }
        [buf[0]]
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
        receive_quote_amount = r_one_sell_base_token(state, pay_base_amount)?;
        new_r = RState::BelowOne;
    } else if state.r == RState::AboveOne {
        let back_to_one_pay_base = state.b_0.checked_sub(state.b)?;
        let back_to_one_receive_quote = state.q.checked_sub(state.q_0)?;

        if pay_base_amount.into_u256_ceil() < back_to_one_pay_base.into_u256_ceil() {
            receive_quote_amount = r_above_sell_base_token(state, pay_base_amount)?;
            new_r = RState::AboveOne;

            if receive_quote_amount.into_u256_ceil() > back_to_one_receive_quote.into_u256_ceil() {
                receive_quote_amount = back_to_one_receive_quote;
            }
        } else if pay_base_amount.into_u256_ceil() == back_to_one_pay_base.into_u256_ceil() {
            receive_quote_amount = back_to_one_receive_quote;
            new_r = RState::One;
        } else {
            receive_quote_amount = back_to_one_receive_quote.checked_add(r_one_sell_base_token(
                state,
                pay_base_amount.checked_sub(back_to_one_pay_base)?,
            )?)?;
            new_r = RState::BelowOne;
        }
    } else {
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
pub fn adjusted_target(state: &mut PMMState) -> ProgramResult {
    if state.r == RState::BelowOne {
        state.q_0 = solve_quadratic_function_for_target(
            state.q,
            state.b.checked_sub(state.b_0)?,
            state.i,
            state.k,
        )?;
    } else if state.r == RState::AboveOne {
        state.b_0 = solve_quadratic_function_for_target(
            state.b,
            state.q.checked_sub(state.q_0)?,
            FixedU256::reciprocal_floor(state.i)?,
            state.k,
        )?;
    }
    Ok(())
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
        bn::{FixedU256, U256},
        math2::solve_quadratic_function_for_target,
        utils::{
            test_utils::{default_i, default_k},
            DEFAULT_TOKEN_DECIMALS,
        },
        v2curve::{
            adjusted_target, get_mid_price, r_above_sell_base_token, r_above_sell_quote_token,
            r_below_sell_base_token, r_below_sell_quote_token, r_one_sell_base_token, PMMState,
            RState,
        },
    };

    #[test]
    fn basic() {
        let k: FixedU256 = default_k();
        let i: FixedU256 = default_i();
        let mut r = RState::One;
        let base_balance: FixedU256 =
            FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap();
        let quote_balance: FixedU256 =
            FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap();
        let base_reserve = base_balance;
        let quote_reserve = quote_balance;
        let base_target: FixedU256 =
            FixedU256::new_from_int(100.into(), DEFAULT_TOKEN_DECIMALS).unwrap();
        let quote_target: FixedU256 =
            FixedU256::new_from_int(10000.into(), DEFAULT_TOKEN_DECIMALS).unwrap();

        let amount: FixedU256 = FixedU256::new_from_int(10.into(), DEFAULT_TOKEN_DECIMALS).unwrap();

        // ============ R = 0 case =============
        let mut state = PMMState::new(
            default_i(),
            default_k(),
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            r,
        );
        adjusted_target(&mut state).unwrap();

        assert_eq!(U256::to_u64(state.b_0.into_u256_ceil()).unwrap(), 100,);

        assert_eq!(U256::to_u64(state.q_0.into_u256_ceil()).unwrap(), 10000,);

        let receive_quote_amount = r_one_sell_base_token(state, amount).unwrap();

        assert_eq!(
            U256::to_u64(receive_quote_amount.into_u256_ceil()).unwrap(),
            909
        );

        // ============ R > 1 cases ============
        r = RState::AboveOne;
        state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            base_target,
            quote_target,
            r,
        );

        let new_b_0 = solve_quadratic_function_for_target(
            state.b,
            state.q.checked_sub(state.q_0).unwrap(),
            FixedU256::reciprocal_floor(state.i).unwrap(),
            state.k,
        )
        .unwrap();

        assert_eq!(U256::to_u64(new_b_0.into_u256_ceil()).unwrap(), 100);

        adjusted_target(&mut state).unwrap();

        assert_eq!(U256::to_u64(state.b_0.into_u256_ceil()).unwrap(), 100);

        assert_eq!(
            r_above_sell_base_token(state, amount).unwrap(),
            FixedU256::new_from_int(950.into(), DEFAULT_TOKEN_DECIMALS).unwrap()
        );

        assert_eq!(
            U256::to_u64(
                r_above_sell_quote_token(state, amount)
                    .unwrap()
                    .into_u256_ceil()
            )
            .unwrap(),
            1
        );

        // ============ R < 1 cases ============
        r = RState::BelowOne;
        state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            base_target,
            quote_target,
            r,
        );

        assert_eq!(
            r_below_sell_base_token(state, amount).unwrap(),
            FixedU256::new_from_int(909.into(), DEFAULT_TOKEN_DECIMALS).unwrap()
        );

        assert_eq!(
            U256::to_u64(
                r_below_sell_quote_token(state, amount)
                    .unwrap()
                    .into_u256_ceil()
            )
            .unwrap(),
            1
        );

        // ============ Helper functions ============
        r = RState::One;
        state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            base_target,
            quote_target,
            r,
        );

        let mut new_state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            base_target,
            quote_target,
            r,
        );
        adjusted_target(&mut new_state).unwrap();
        assert_eq!(new_state, state);

        let value = FixedU256::new_from_int(1000.into(), DEFAULT_TOKEN_DECIMALS)
            .unwrap()
            .checked_div_floor(FixedU256::new(10.into()))
            .unwrap();
        assert_eq!(get_mid_price(state).unwrap(), value);
    }
}
