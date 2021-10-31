//! Calculation functions

use crate::{
    error::SwapError,
    math::{Decimal, TryAdd, TryDiv, TryMul, TrySub},
};
use solana_program::program_error::ProgramError;

/// Get target amount given quote amount.
///
/// target_amount = market_price * quote_amount * (1 - slope
///         + slope * (target_reserve^2 / future_reserve / current_reserve))
/// where quote_amount = future_reserve - current_reserve.
///
/// # Arguments
///
/// * target_reserve - initial reserve position to track divergent loss.
/// * future_reserve - reserve position after the current quoted trade.
/// * current_reserve - current reserve position.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// target amount determined by the pricing function.
pub fn get_target_amount(
    target_reserve: Decimal,
    future_reserve: Decimal,
    current_reserve: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if current_reserve <= Decimal::zero()
        || future_reserve < current_reserve
        || future_reserve > target_reserve
    {
        return Err(SwapError::CalculationFailure.into());
    }

    let fair_amount = future_reserve
        .try_sub(current_reserve)?
        .try_mul(market_price)?;

    if slope.lt(&Decimal::zero()) || slope.gt(&Decimal::one()) {
        return Err(SwapError::InvalidSlope.into());
    }

    if slope.is_zero() {
        return Ok(fair_amount);
    }
    let penalty_ratio = target_reserve
        .try_mul(target_reserve)?
        .try_div(future_reserve)?
        .try_div(current_reserve)?;
    let penalty = penalty_ratio.try_mul(slope)?;
    fair_amount.try_mul(penalty.try_add(Decimal::one())?.try_sub(slope)?)
}

/// Get target amount given quote amount in reserve direction.
///
/// # Arguments
///
/// * target_reserve - initial reserve position to track divergent loss.
/// * current_reserve - current reserve position.
/// * quote_amount - quote amount.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// target amount determined by the pricing function.
pub fn get_target_amount_reverse_direction(
    target_reserve: Decimal,
    current_reserve: Decimal,
    quote_amount: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if target_reserve <= Decimal::zero() {
        return Err(SwapError::CalculationFailure.into());
    }

    if quote_amount.is_zero() {
        return Ok(Decimal::zero());
    }

    if slope.lt(&Decimal::zero()) || slope.gt(&Decimal::one()) {
        return Err(SwapError::InvalidSlope.into());
    }

    let fair_amount = quote_amount.try_mul(market_price)?;
    if slope.is_zero() {
        return Ok(fair_amount.min(current_reserve));
    }

    if slope == Decimal::one() {
        let adjusted_ratio = if fair_amount.is_zero() {
            Decimal::zero()
        } else if fair_amount.try_mul(current_reserve)?.try_div(fair_amount)? == current_reserve {
            fair_amount
                .try_mul(current_reserve)?
                .try_div(target_reserve)?
                .try_div(target_reserve)?
        } else {
            quote_amount
                .try_mul(current_reserve)?
                .try_div(target_reserve)?
                .try_mul(market_price)?
                .try_div(target_reserve)?
        };

        return current_reserve
            .try_mul(adjusted_ratio)?
            .try_div(adjusted_ratio.try_add(Decimal::one())?);
    }

    let future_reserve = slope
        .try_mul(target_reserve)?
        .try_div(current_reserve)?
        .try_mul(target_reserve)?
        .try_add(fair_amount)?;
    let mut adjusted_reserve = Decimal::one().try_sub(slope)?.try_mul(current_reserve)?;

    let is_smaller = if adjusted_reserve < future_reserve {
        adjusted_reserve = future_reserve.try_sub(adjusted_reserve)?;
        true
    } else {
        adjusted_reserve = adjusted_reserve.try_sub(future_reserve)?;
        false
    };
    adjusted_reserve = Decimal::from(adjusted_reserve.try_floor_u64()?);

    let square_root = Decimal::one()
        .try_sub(slope)?
        .try_mul(4)?
        .try_mul(slope)?
        .try_mul(target_reserve)?
        .try_mul(target_reserve)?;
    let square_root = adjusted_reserve
        .try_mul(adjusted_reserve)?
        .try_add(square_root)?
        .sqrt()?;

    let denominator = Decimal::one().try_sub(slope)?.try_mul(2)?;
    let numerator = if is_smaller {
        square_root.try_sub(adjusted_reserve)?
    } else {
        adjusted_reserve.try_add(square_root)?
    };

    let candidate_reserve = numerator.try_div(denominator)?;
    if candidate_reserve > current_reserve {
        Ok(Decimal::zero())
    } else {
        current_reserve.try_sub(candidate_reserve)
    }
}

/// Get adjusted target reserve given quote amount.
///
/// # Arguments
///
/// * current_reserve - current reserve position.
/// * quote_amount - quote amount.
/// * market price - fair market price determined by internal and external oracle.
/// * slope - the higher the curve slope is, the bigger the price splippage.
///
/// # Return value
///
/// adjusted target reserve.
pub fn get_target_reserve(
    current_reserve: Decimal,
    quote_amount: Decimal,
    market_price: Decimal,
    slope: Decimal,
) -> Result<Decimal, ProgramError> {
    if current_reserve.is_zero() {
        return Ok(Decimal::zero());
    }
    if slope.is_zero() {
        return quote_amount.try_mul(market_price)?.try_add(current_reserve);
    }

    if slope.lt(&Decimal::zero()) || slope.gt(&Decimal::one()) {
        return Err(SwapError::InvalidSlope.into());
    }

    let price_offset = market_price.try_mul(slope)?.try_mul(4)?;

    let square_root = if price_offset.is_zero() {
        Decimal::one()
    } else if price_offset.try_mul(quote_amount)?.try_div(price_offset)? == quote_amount {
        price_offset
            .try_mul(quote_amount)?
            .try_div(current_reserve)?
            .try_add(Decimal::one())?
            .sqrt()?
    } else {
        price_offset
            .try_div(current_reserve)?
            .try_mul(quote_amount)?
            .try_add(Decimal::one())?
            .sqrt()?
    };

    let premium = square_root
        .try_sub(Decimal::one())?
        .try_div(2)?
        .try_div(slope)?
        .try_add(Decimal::one())?;

    premium.try_mul(current_reserve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{default_market_price, default_slope};
    use proptest::prelude::*;

    prop_compose! {
        fn get_reserve_and_amount()(next_value in 1..=u16::MAX-1)(
            target_reserve in next_value..=u16::MAX,
            current_reserve in 0..=next_value,
            quote_amount in 0..=u16::MAX
        ) -> (Decimal, Decimal, Decimal) {
            (Decimal::from(target_reserve as u64), Decimal::from(current_reserve as u64), Decimal::from(quote_amount as u64))
        }
    }

    prop_compose! {
        fn get_reserve_range()(next_value in 1..=u32::MAX/2-1)(
            target_reserve in next_value * 2..=u32::MAX,
            future_reserve in next_value..=next_value * 2,
            current_reserve in 1..=next_value
        ) -> (Decimal, Decimal, Decimal) {
            (Decimal::from(target_reserve as u64), Decimal::from(future_reserve as u64), Decimal::from(current_reserve as u64))
        }
    }

    proptest! {
        #[test]
        fn test_get_target_amount_reverse_direction(
            (target_reserve, current_reserve, quote_amount) in get_reserve_and_amount()
        ) {
            let slope: Decimal = default_slope();
            let market_price: Decimal = default_market_price();
            if !target_reserve.is_zero() && current_reserve > Decimal::zero()  {
                let expected_amount = if quote_amount.is_zero() {
                    Decimal::zero()
                } else {
                    let fair_amount = quote_amount.try_mul(market_price)?;
                    if slope.is_zero() {
                        fair_amount.min(current_reserve)
                    } else if slope == Decimal::one() {
                        let adjusted_ratio = if fair_amount.is_zero() {
                            Decimal::zero()
                        } else if fair_amount.try_mul(current_reserve)?.try_div(fair_amount)? == current_reserve {
                            fair_amount.try_mul(current_reserve)?.try_div(target_reserve)?.try_div(target_reserve)?
                        } else {
                            quote_amount.try_mul(current_reserve)?.try_div(target_reserve)?.try_mul(market_price)?.try_div(target_reserve)?
                        };
                        current_reserve
                            .try_mul(adjusted_ratio)?
                            .try_div(adjusted_ratio.try_add(Decimal::one())?)?
                    } else {
                        let future_reserve = slope.try_mul(target_reserve)?.try_div(current_reserve)?.try_mul(target_reserve)?.try_add(fair_amount)?;
                        let mut adjusted_reserve = Decimal::one().try_sub(slope)?.try_mul(current_reserve)?;
                        let is_smaller = if adjusted_reserve < future_reserve {
                            adjusted_reserve = future_reserve.try_sub(adjusted_reserve)?;
                            true
                        } else {
                            adjusted_reserve = adjusted_reserve.try_sub(future_reserve)?;
                            false
                        };
                        adjusted_reserve = Decimal::from(adjusted_reserve.try_floor_u64()?);

                        let square_root = Decimal::one()
                            .try_sub(slope)?
                            .try_mul(4)?
                            .try_mul(slope)?
                            .try_mul(target_reserve)?
                            .try_mul(target_reserve)?;
                        let square_root = adjusted_reserve
                            .try_mul(adjusted_reserve)?
                            .try_add(square_root)?
                            .sqrt()?;

                        let denominator = Decimal::one().try_sub(slope)?.try_mul(2)?;
                        let numerator = if is_smaller {
                            square_root.try_sub(adjusted_reserve)?
                        } else {
                            adjusted_reserve.try_add(square_root)?
                        };

                        let target_reserve = numerator.try_div(denominator)?;
                        if target_reserve > current_reserve {
                            Decimal::zero()
                        } else {
                            current_reserve.try_sub(target_reserve)?
                        }
                    }
                };
                assert_eq!(
                    expected_amount,
                    get_target_amount_reverse_direction(
                        target_reserve,
                        current_reserve,
                        quote_amount,
                        market_price,
                        slope
                    )?
                );
            }
        }

        fn test_get_target_reserve(
            (_target_reserve, current_reserve, quote_amount) in get_reserve_and_amount()
        ) {
            let slope: Decimal = default_slope();
            let market_price: Decimal = default_market_price();
            let expected_target_reserve = if current_reserve.is_zero() {
                Decimal::zero()
            } else if slope.is_zero() {
                quote_amount.try_mul(market_price)?.try_add(current_reserve)?
            } else {
                let price_offset = market_price
                        .try_mul(slope)?
                        .try_mul(4)?;
                    let square_root = if price_offset.is_zero() {
                        Decimal::one()
                    } else if price_offset
                        .try_mul(quote_amount)?
                        .try_div(price_offset)? == quote_amount
                    {
                        price_offset
                            .try_mul(quote_amount)?
                            .try_div(current_reserve)?
                            .try_add(Decimal::one())?
                            .sqrt()?
                    } else {
                        price_offset
                            .try_div(current_reserve)?
                            .try_mul(quote_amount)?
                            .try_add(Decimal::one())?
                            .sqrt()?
                    };

                    let premium = square_root
                        .try_sub(Decimal::one())?
                        .try_div(2)?
                        .try_div(slope)?
                        .try_add(Decimal::one())?;

                    premium.try_mul(current_reserve)?
            };

            assert_eq!(
                expected_target_reserve,
                get_target_reserve(
                    current_reserve,
                    quote_amount,
                    market_price,
                    slope
                )?
            );
        }

        #[test]
        fn test_get_target_amount(
            (target_reserve, future_reserve, current_reserve) in get_reserve_range()
        ) {
            let slope: Decimal = default_slope();
            let market_price: Decimal = default_market_price();
            let fair_amount = future_reserve
                .try_sub(current_reserve)?
                .try_mul(market_price)?;
            let expected_target_amount: Decimal = if slope.is_zero() {
                fair_amount
            } else {
                let penalty_ratio = target_reserve
                    .try_mul(target_reserve)?
                    .try_div(future_reserve)?
                    .try_div(current_reserve)?;
                let penalty = penalty_ratio.try_mul(slope)?;
                fair_amount.try_mul(penalty.try_add(Decimal::one())?.try_sub(slope)?)?
            };

            assert_eq!(
                expected_target_amount,
                get_target_amount(
                    target_reserve,
                    future_reserve,
                    current_reserve,
                    market_price,
                    slope
                )?
            );
        }
    }

    #[test]
    fn test_basics() {
        let target_reserve = Decimal::from(2_000_000u64);
        let current_reserve = Decimal::from(1_000_000u64);
        let quote_amount = Decimal::from(3_000u64);
        let slope: Decimal = default_slope();
        let market_price: Decimal = default_market_price();

        // Test failures on get_target_amount_reverse_direction
        {
            assert!(get_target_amount_reverse_direction(
                target_reserve,
                Decimal::zero(),
                quote_amount,
                market_price,
                slope
            )
            .is_err());
        }

        {
            assert_eq!(
                get_target_amount_reverse_direction(
                    target_reserve,
                    current_reserve,
                    Decimal::zero(),
                    market_price,
                    slope
                )
                .unwrap(),
                Decimal::zero()
            );

            let fair_amount = quote_amount.try_mul(market_price).unwrap();
            assert_eq!(
                get_target_amount_reverse_direction(
                    target_reserve,
                    current_reserve,
                    quote_amount,
                    market_price,
                    Decimal::zero()
                )
                .unwrap(),
                fair_amount.min(current_reserve)
            );

            let adjusted_ratio = fair_amount
                .try_mul(current_reserve)
                .unwrap()
                .try_div(target_reserve)
                .unwrap()
                .try_div(target_reserve)
                .unwrap();
            let expected_amount = current_reserve
                .try_mul(adjusted_ratio)
                .unwrap()
                .try_div(adjusted_ratio.try_add(Decimal::one()).unwrap())
                .unwrap();
            assert_eq!(
                get_target_amount_reverse_direction(
                    target_reserve,
                    current_reserve,
                    quote_amount,
                    market_price,
                    Decimal::one()
                )
                .unwrap(),
                expected_amount
            );
        }

        {
            assert_eq!(
                get_target_reserve(Decimal::zero(), quote_amount, market_price, slope).unwrap(),
                Decimal::zero()
            );
        }

        {
            let expected_amount = quote_amount
                .try_mul(market_price)
                .unwrap()
                .try_add(current_reserve)
                .unwrap();
            assert_eq!(
                get_target_reserve(current_reserve, quote_amount, market_price, Decimal::zero())
                    .unwrap(),
                expected_amount
            );
        }

        let small = Decimal::from(1_000_000u64);
        let medium = Decimal::from(2_000_000u64);
        let large = Decimal::from(3_000_000u64);
        // test failure cases for get_target_amount
        {
            assert!(
                get_target_amount(large, medium, Decimal::zero(), market_price, slope).is_err()
            );

            assert!(get_target_amount(small, medium, large, market_price, slope).is_err());

            assert!(
                get_target_amount(Decimal::zero(), medium, large, market_price, slope).is_err()
            );
        }

        // test case for slope = 0 on get_target_amount
        {
            let fair_amount = medium
                .try_sub(small)
                .unwrap()
                .try_mul(market_price)
                .unwrap();
            assert_eq!(
                get_target_amount(large, medium, small, market_price, Decimal::zero()).unwrap(),
                fair_amount
            );
        }
    }
}
