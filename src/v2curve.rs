//! pricing for proactive market maker
use std::{cmp::Ordering, mem::size_of};

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{program_error::ProgramError, program_pack::Sealed};

use crate::{
    bn::FixedU64,
    math2::{
        general_integrate, solve_quadratic_function_for_target, solve_quadratic_function_for_trade,
    },
    solana_program::program_pack::Pack,
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
    /// i - mid price
    pub i: FixedU64,

    /// k - slope
    pub k: FixedU64,

    /// b - base_reserve
    pub b: FixedU64,

    /// q - quote_reserve
    pub q: FixedU64,

    /// b_0 - base_target
    pub b_0: FixedU64,

    /// q_0 - quote_target
    pub q_0: FixedU64,

    /// r - state
    pub r: RState,
}

impl PMMState {
    /// initialize PMMState
    #[allow(clippy::many_single_char_names)]
    pub fn new(
        i: FixedU64,
        k: FixedU64,
        b: FixedU64,
        q: FixedU64,
        b_0: FixedU64,
        q_0: FixedU64,
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

    // ================== buy & sell ===================

    /// Return receive_quote_amount and r_status
    pub fn sell_base_token(
        &self,
        pay_base_amount: FixedU64,
    ) -> Result<(FixedU64, RState), ProgramError> {
        let mut receive_quote_amount;
        let new_r;
        if self.r == RState::One {
            receive_quote_amount = self.r_one_sell_base_token(pay_base_amount)?;
            new_r = RState::BelowOne;
        } else if self.r == RState::AboveOne {
            let back_to_one_pay_base = self.b_0.checked_sub(self.b)?;
            let back_to_one_receive_quote = self.q.checked_sub(self.q_0)?;

            match pay_base_amount
                .into_real_u64_ceil()
                .cmp(&back_to_one_pay_base.into_real_u64_ceil())
            {
                Ordering::Less => {
                    receive_quote_amount = self.r_above_sell_base_token(pay_base_amount)?;
                    new_r = RState::AboveOne;

                    if receive_quote_amount.into_real_u64_ceil()
                        > back_to_one_receive_quote.into_real_u64_ceil()
                    {
                        receive_quote_amount = back_to_one_receive_quote;
                    }
                }
                Ordering::Equal => {
                    receive_quote_amount = back_to_one_receive_quote;
                    new_r = RState::One;
                }
                Ordering::Greater => {
                    receive_quote_amount =
                        back_to_one_receive_quote.checked_add(self.r_one_sell_base_token(
                            pay_base_amount.checked_sub(back_to_one_pay_base)?,
                        )?)?;
                    new_r = RState::BelowOne;
                }
            }
        } else {
            receive_quote_amount = self.r_below_sell_base_token(pay_base_amount)?;
            new_r = RState::BelowOne;
        }

        Ok((receive_quote_amount, new_r))
    }

    /// Return receive_base_amount and r_status
    pub fn sell_quote_token(
        &self,
        pay_quote_amount: FixedU64,
    ) -> Result<(FixedU64, RState), ProgramError> {
        let mut receive_base_amount;
        let new_r;
        if self.r == RState::One {
            receive_base_amount = self.r_one_sell_quote_token(pay_quote_amount)?;
            new_r = RState::AboveOne;
        } else if self.r == RState::AboveOne {
            receive_base_amount = self.r_above_sell_quote_token(pay_quote_amount)?;
            new_r = RState::AboveOne;
        } else {
            let back_to_one_pay_quote = self.q_0.checked_sub(self.q)?;
            let back_to_one_receive_base = self.b.checked_sub(self.b_0)?;

            match pay_quote_amount
                .into_real_u64_ceil()
                .cmp(&back_to_one_pay_quote.into_real_u64_ceil())
            {
                Ordering::Less => {
                    receive_base_amount = self.r_below_sell_quote_token(pay_quote_amount)?;
                    new_r = RState::BelowOne;

                    if receive_base_amount.into_real_u64_ceil()
                        > back_to_one_receive_base.into_real_u64_ceil()
                    {
                        receive_base_amount = back_to_one_receive_base;
                    }
                }
                Ordering::Equal => {
                    receive_base_amount = back_to_one_receive_base;
                    new_r = RState::One;
                }
                Ordering::Greater => {
                    receive_base_amount =
                        back_to_one_receive_base.checked_add(self.r_one_sell_quote_token(
                            pay_quote_amount.checked_sub(back_to_one_pay_quote)?,
                        )?)?;
                    new_r = RState::AboveOne;
                }
            }
        }

        Ok((receive_base_amount, new_r))
    }

    // ============ R = 1 cases ============

    /// receiveQuoteToken
    pub fn r_one_sell_base_token(
        &self,
        pay_base_amount: FixedU64,
    ) -> Result<FixedU64, ProgramError> {
        solve_quadratic_function_for_trade(self.q_0, self.q_0, pay_base_amount, self.i, self.k)
    }

    /// receiveBaseToken
    pub fn r_one_sell_quote_token(
        &self,
        pay_quote_amount: FixedU64,
    ) -> Result<FixedU64, ProgramError> {
        solve_quadratic_function_for_trade(
            self.b_0,
            self.b_0,
            pay_quote_amount,
            FixedU64::reciprocal_floor(self.i)?,
            self.k,
        )
    }

    // ============ R < 1 cases ============

    /// receiveBaseToken
    pub fn r_below_sell_quote_token(
        &self,
        pay_quote_amount: FixedU64,
    ) -> Result<FixedU64, ProgramError> {
        general_integrate(
            self.q_0,
            self.q.checked_add(pay_quote_amount)?,
            self.q,
            FixedU64::reciprocal_floor(self.i)?,
            self.k,
        )
    }

    /// receiveQuoteToken
    pub fn r_below_sell_base_token(
        &self,
        pay_base_amount: FixedU64,
    ) -> Result<FixedU64, ProgramError> {
        solve_quadratic_function_for_trade(self.q_0, self.q, pay_base_amount, self.i, self.k)
    }

    // ============ R > 1 cases ============

    /// receiveQuoteToken
    pub fn r_above_sell_base_token(
        &self,
        pay_base_amount: FixedU64,
    ) -> Result<FixedU64, ProgramError> {
        general_integrate(
            self.b_0,
            self.b.checked_add(pay_base_amount)?,
            self.b,
            self.i,
            self.k,
        )
    }

    /// receiveBaseToken
    pub fn r_above_sell_quote_token(
        &self,
        pay_quote_amount: FixedU64,
    ) -> Result<FixedU64, ProgramError> {
        solve_quadratic_function_for_trade(
            self.b_0,
            self.b,
            pay_quote_amount,
            FixedU64::reciprocal_floor(self.i)?,
            self.k,
        )
    }

    // ============ Helper functions ============

    /// adjust target value
    pub fn adjusted_target(&self) -> Result<Self, ProgramError> {
        let mut q_0 = self.q_0;
        let mut b_0 = self.b_0;
        if self.r == RState::BelowOne {
            q_0 = solve_quadratic_function_for_target(
                self.q,
                self.b.checked_sub(self.b_0)?,
                self.i,
                self.k,
            )?;
        } else if self.r == RState::AboveOne {
            b_0 = solve_quadratic_function_for_target(
                self.b,
                self.q.checked_sub(self.q_0)?,
                FixedU64::reciprocal_floor(self.i)?,
                self.k,
            )?;
        }
        Ok(Self {
            i: self.i,
            k: self.k,
            b: self.b,
            q: self.q,
            b_0,
            q_0,
            r: self.r,
        })
    }

    /// get mid price
    pub fn get_mid_price(&self) -> Result<FixedU64, ProgramError> {
        if self.r == RState::BelowOne {
            let mut r = self
                .q_0
                .checked_mul_floor(self.q_0)?
                .checked_div_floor(self.q)?
                .checked_div_floor(self.q)?;
            r = FixedU64::one()
                .checked_sub(self.k)?
                .checked_add(self.k.checked_mul_floor(r)?)?;

            self.i.checked_div_floor(r)
        } else {
            let mut r = self
                .b_0
                .checked_mul_floor(self.b_0)?
                .checked_div_floor(self.b)?
                .checked_div_floor(self.b)?;
            r = FixedU64::one()
                .checked_sub(self.k)?
                .checked_add(self.k.checked_mul_floor(r)?)?;

            self.i.checked_mul_floor(r)
        }
    }
}

impl Sealed for PMMState {}
impl Pack for PMMState {
    const LEN: usize = 55;
    #[allow(clippy::many_single_char_names)]
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 55];
        #[allow(clippy::ptr_offset_with_cast)]
        let (i, k, b, q, b_0, q_0, r) = array_refs![input, 9, 9, 9, 9, 9, 9, 1];
        Ok(Self {
            i: FixedU64::unpack_from_slice(i)?,
            k: FixedU64::unpack_from_slice(k)?,
            b: FixedU64::unpack_from_slice(b)?,
            q: FixedU64::unpack_from_slice(q)?,
            b_0: FixedU64::unpack_from_slice(b_0)?,
            q_0: FixedU64::unpack_from_slice(q_0)?,
            r: RState::unpack(r)?,
        })
    }

    #[allow(clippy::many_single_char_names)]
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 55];
        let (i, k, b, q, b_0, q_0, r) = mut_array_refs![output, 9, 9, 9, 9, 9, 9, 1];
        self.k.pack_into_slice(&mut k[..]);
        self.i.pack_into_slice(&mut i[..]);
        *r = self.r.pack();
        self.b.pack_into_slice(&mut b[..]);
        self.q.pack_into_slice(&mut q[..]);
        self.b_0.pack_into_slice(&mut b_0[..]);
        self.q_0.pack_into_slice(&mut q_0[..]);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bn::FixedU64,
        math2::solve_quadratic_function_for_target,
        utils::{
            test_utils::{default_i, default_k},
            DEFAULT_TOKEN_DECIMALS,
        },
        v2curve::{PMMState, RState},
    };

    #[test]
    fn basic() {
        let k: FixedU64 = default_k();
        let i: FixedU64 = default_i();
        let mut r = RState::One;
        let base_balance: FixedU64 = FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap();
        let quote_balance: FixedU64 =
            FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap();
        let base_reserve = base_balance;
        let quote_reserve = quote_balance;
        let base_target: FixedU64 = FixedU64::new_from_int(100, DEFAULT_TOKEN_DECIMALS).unwrap();
        let quote_target: FixedU64 = FixedU64::new_from_int(10000, DEFAULT_TOKEN_DECIMALS).unwrap();

        let amount: FixedU64 = FixedU64::new_from_int(10, DEFAULT_TOKEN_DECIMALS).unwrap();

        // ============ R = 0 case =============
        let state = PMMState::new(
            default_i(),
            default_k(),
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            r,
        );
        let mut state = state.adjusted_target().unwrap();

        assert_eq!(state.b_0.into_real_u64_ceil(), 100);

        assert_eq!(state.q_0.into_real_u64_ceil(), 10000);

        let receive_quote_amount = state.r_one_sell_base_token(amount).unwrap();

        assert_eq!(receive_quote_amount.into_real_u64_ceil(), 910);

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
            FixedU64::reciprocal_floor(state.i).unwrap(),
            state.k,
        )
        .unwrap();

        assert_eq!(state.b.into_real_u64_ceil(), 100);
        assert_eq!(state.q.into_real_u64_ceil(), 10000);
        assert_eq!(state.q_0.into_real_u64_ceil(), 10000);
        assert_eq!(
            state.q.checked_sub(state.q_0).unwrap().into_real_u64_ceil(),
            0
        );

        assert_eq!(new_b_0.into_real_u64_ceil(), 100);

        let mut state = state.adjusted_target().unwrap();

        assert_eq!(state.b_0.into_real_u64_ceil(), 100);

        assert_eq!(
            state
                .r_above_sell_base_token(amount)
                .unwrap()
                .into_real_u64_ceil(),
            955
        );

        assert_eq!(
            state
                .r_above_sell_quote_token(amount)
                .unwrap()
                .into_real_u64_ceil(),
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
            state
                .r_below_sell_base_token(amount)
                .unwrap()
                .into_real_u64_ceil(),
            910
        );

        assert_eq!(
            state
                .r_below_sell_quote_token(amount)
                .unwrap()
                .into_real_u64_ceil(),
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

        let new_state = PMMState::new(
            i,
            k,
            base_balance,
            quote_balance,
            base_target,
            quote_target,
            r,
        );
        let new_state = new_state.adjusted_target().unwrap();
        assert_eq!(new_state, state);

        let value = FixedU64::new_from_int(1000, DEFAULT_TOKEN_DECIMALS)
            .unwrap()
            .checked_div_floor(FixedU64::new(10))
            .unwrap();
        assert_eq!(state.get_mid_price().unwrap(), value);
    }
}
