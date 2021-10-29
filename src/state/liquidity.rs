use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::UnixTimestamp,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

use crate::{
    error::SwapError,
    math::{Decimal, TryDiv, TryMul},
    state::unpack_bool,
};

use std::convert::TryFrom;

/// Max number of positions
pub const MAX_LIQUIDITY_POSITIONS: usize = 10;
/// Min period towards next claim
pub const MIN_CLAIM_PERIOD: UnixTimestamp = 2592000;

/// Liquidity user info
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidityProvider {
    /// Initialization status
    pub is_initialized: bool,
    /// Owner authority
    pub owner: Pubkey,
    /// Liquidity positions owned by this user
    pub positions: Vec<LiquidityPosition>,
}

impl LiquidityProvider {
    /// Constructor to create new liquidity provider
    ///
    /// # Arguments
    ///
    /// * owner - liquidity provider owner address.
    /// * positions - liquidity provider's current position.
    ///
    /// # Return value
    ///
    /// liquidity provider
    pub fn new(owner: Pubkey, positions: Vec<LiquidityPosition>) -> Self {
        let mut provider = Self::default();
        provider.init(owner, positions);
        provider
    }

    /// Initialize a liquidity provider
    ///
    /// # Arguments
    ///
    /// * owner - liquidity provider owner address.
    /// * positions - liquidity provider's current position.
    pub fn init(&mut self, owner: Pubkey, positions: Vec<LiquidityPosition>) {
        self.is_initialized = true;
        self.owner = owner;
        self.positions = positions;
    }

    /// Find position by pool
    ///
    /// # Arguments
    ///
    /// * pool - pool address.
    ///
    /// # Return value
    ///
    /// liquidity position, position index
    pub fn find_position(
        &mut self,
        pool: Pubkey,
    ) -> Result<(&mut LiquidityPosition, usize), ProgramError> {
        if self.positions.is_empty() {
            return Err(SwapError::LiquidityPositionEmpty.into());
        }
        let position_index = self
            .find_position_index(pool)
            .ok_or(SwapError::InvalidPositionKey)?;
        Ok((
            self.positions.get_mut(position_index).unwrap(),
            position_index,
        ))
    }

    /// Find or add position by pool
    ///
    /// # Arguments
    ///
    /// * pool - pool address.
    /// * current_ts - unix time stamp
    ///
    /// # Return value
    ///
    /// liquidity position
    pub fn find_or_add_position(
        &mut self,
        pool: Pubkey,
        current_ts: UnixTimestamp,
    ) -> Result<&mut LiquidityPosition, ProgramError> {
        if let Some(position_index) = self.find_position_index(pool) {
            return Ok(&mut self.positions[position_index]);
        }
        let position = LiquidityPosition::new(pool, current_ts).unwrap();
        self.positions.push(position);
        Ok(self.positions.last_mut().unwrap())
    }

    /// Find position index given pool address
    ///
    /// # Arguments
    ///
    /// * pool - pool address.
    ///
    /// # Return value
    ///
    /// pool position index
    fn find_position_index(&self, pool: Pubkey) -> Option<usize> {
        self.positions
            .iter()
            .position(|position| position.pool == pool)
    }

    /// Withdraw liquidity and remove it from deposits if zeroed out
    ///
    /// # Arguments
    ///
    /// * withdraw_amount - amount to withdraw from the pool.
    /// * position_index - pool position index
    ///
    /// # Return value
    ///
    /// withdraw status
    pub fn withdraw(&mut self, withdraw_amount: u64, position_index: usize) -> ProgramResult {
        let position = &mut self.positions[position_index];
        if withdraw_amount == position.liquidity_amount && position.rewards_owed == 0 {
            self.positions.remove(position_index);
        } else {
            position.withdraw(withdraw_amount)?;
        }
        Ok(())
    }

    /// Claim rewards in corresponding position
    ///
    /// # Arguments
    ///
    /// * pool - pool address.
    ///
    /// # Return value
    ///
    /// claimed amount
    pub fn claim(&mut self, pool: Pubkey) -> Result<u64, ProgramError> {
        let (position, position_index) = self.find_position(pool)?;
        let claimed_amount = position.claim_rewards()?;
        if position.liquidity_amount == 0 && position.rewards_estimated == 0 {
            self.positions.remove(position_index);
        }
        Ok(claimed_amount)
    }
}

/// Liquidity position of a pool
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidityPosition {
    /// Swap pool address
    pub pool: Pubkey,
    /// Amount of liquidity owned by this position
    pub liquidity_amount: u64,
    /// Rewards amount owed
    pub rewards_owed: u64,
    /// Rewards amount estimated in new claim period
    pub rewards_estimated: u64,
    /// Cumulative interest
    pub cumulative_interest: u64,
    /// Last updated timestamp
    pub last_update_ts: UnixTimestamp,
    /// Next claim timestamp
    pub next_claim_ts: UnixTimestamp,
}

impl LiquidityPosition {
    /// Create new liquidity position
    ///
    /// # Arguments
    ///
    /// * pool - pool address.
    /// * current_ts - unix timestamp
    ///
    /// # Return value
    ///
    /// liquidity position
    pub fn new(pool: Pubkey, current_ts: UnixTimestamp) -> Result<Self, ProgramError> {
        Ok(Self {
            pool,
            liquidity_amount: 0,
            rewards_owed: 0,
            rewards_estimated: 0,
            cumulative_interest: 0,
            last_update_ts: current_ts,
            next_claim_ts: current_ts
                .checked_add(MIN_CLAIM_PERIOD)
                .ok_or(SwapError::CalculationFailure)?,
        })
    }

    /// Deposit liquidity
    ///
    /// # Arguments
    ///
    /// * deposit_amount - amount to deposit.
    ///
    /// # Return value
    ///
    /// deposit status
    pub fn deposit(&mut self, deposit_amount: u64) -> ProgramResult {
        self.liquidity_amount = self
            .liquidity_amount
            .checked_add(deposit_amount)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Withdraw liquidity
    ///
    /// # Arguments
    ///
    /// * withdraw_amount - amount to withdraw.
    ///
    /// # Return value
    ///
    /// withdraw status
    pub fn withdraw(&mut self, withdraw_amount: u64) -> ProgramResult {
        if withdraw_amount > self.liquidity_amount {
            return Err(SwapError::InsufficientLiquidity.into());
        }
        self.liquidity_amount = self
            .liquidity_amount
            .checked_sub(withdraw_amount)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Update next claim timestamp
    ///
    /// # Return value
    ///
    /// timestamp update status
    pub fn update_claim_ts(&mut self) -> ProgramResult {
        if self.liquidity_amount != 0 {
            self.next_claim_ts = self
                .next_claim_ts
                .checked_add(MIN_CLAIM_PERIOD)
                .ok_or(SwapError::CalculationFailure)?;
        }
        Ok(())
    }

    /// Calculate and update rewards
    ///
    /// # Arguments
    ///
    /// * rewards_ratio - rewards ratio calculated by lp token and deltafi token price.
    /// * current_ts - current unix timestamp.
    ///
    /// # Return value
    ///
    /// reward update status
    pub fn calc_and_update_rewards(
        &mut self,
        rewards_ratio: Decimal,
        current_ts: UnixTimestamp,
    ) -> ProgramResult {
        let calc_period = current_ts
            .checked_sub(self.last_update_ts)
            .ok_or(SwapError::CalculationFailure)?;
        if calc_period > 0 {
            self.rewards_estimated = rewards_ratio
                .try_mul(self.liquidity_amount)?
                .try_div(u64::try_from(MIN_CLAIM_PERIOD).unwrap())?
                .try_mul(u64::try_from(calc_period).unwrap())?
                .try_floor_u64()?
                .checked_add(self.rewards_estimated)
                .ok_or(SwapError::CalculationFailure)?;

            self.last_update_ts = current_ts;
        }

        if current_ts >= self.next_claim_ts {
            self.rewards_owed = self
                .rewards_owed
                .checked_add(self.rewards_estimated)
                .ok_or(SwapError::CalculationFailure)?;
            self.rewards_estimated = 0;
            self.update_claim_ts()?;
        }
        Ok(())
    }

    /// Claim rewards owed
    ///
    /// # Return value
    ///
    /// claimed rewards
    pub fn claim_rewards(&mut self) -> Result<u64, ProgramError> {
        if self.rewards_owed == 0 {
            return Err(SwapError::InsufficientClaimAmount.into());
        }
        self.cumulative_interest = self
            .cumulative_interest
            .checked_add(self.rewards_owed)
            .ok_or(SwapError::CalculationFailure)?;
        let ret = self.rewards_owed;
        self.rewards_owed = 0;
        Ok(ret)
    }
}

impl Sealed for LiquidityProvider {}
impl IsInitialized for LiquidityProvider {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

#[doc(hidden)]
const LIQUIDITY_POSITION_SIZE: usize = 80; // 32 + 8 + 8 + 8 + 8 + 8 + 8
const LIQUIDITY_PROVIDER_SIZE: usize = 834; // 1 + 32 + 1 + (80 * 10)

impl Pack for LiquidityProvider {
    const LEN: usize = LIQUIDITY_PROVIDER_SIZE;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, LIQUIDITY_PROVIDER_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, owner, positions_len, data_flat) = mut_array_refs![
            output,
            1,
            PUBKEY_BYTES,
            1,
            LIQUIDITY_POSITION_SIZE * MAX_LIQUIDITY_POSITIONS
        ];
        is_initialized[0] = self.is_initialized as u8;
        owner.copy_from_slice(self.owner.as_ref());
        *positions_len = u8::try_from(self.positions.len()).unwrap().to_le_bytes();

        let mut offset = 0;
        for position in &self.positions {
            let position_flat = array_mut_ref![data_flat, offset, LIQUIDITY_POSITION_SIZE];
            #[allow(clippy::ptr_offset_with_cast)]
            let (
                pool,
                liquidity_amount,
                rewards_owed,
                rewards_estimated,
                cumulative_interest,
                last_update_ts,
                next_claim_ts,
            ) = mut_array_refs![position_flat, PUBKEY_BYTES, 8, 8, 8, 8, 8, 8];

            pool.copy_from_slice(position.pool.as_ref());
            *liquidity_amount = position.liquidity_amount.to_le_bytes();
            *rewards_owed = position.rewards_owed.to_le_bytes();
            *rewards_estimated = position.rewards_estimated.to_le_bytes();
            *cumulative_interest = position.cumulative_interest.to_le_bytes();
            *last_update_ts = position.last_update_ts.to_le_bytes();
            *next_claim_ts = position.next_claim_ts.to_le_bytes();
            offset += LIQUIDITY_POSITION_SIZE;
        }
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, LIQUIDITY_PROVIDER_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, owner, positions_len, data_flat) = array_refs![
            input,
            1,
            PUBKEY_BYTES,
            1,
            LIQUIDITY_POSITION_SIZE * MAX_LIQUIDITY_POSITIONS
        ];

        let is_initialized = unpack_bool(is_initialized)?;
        let positions_len = u8::from_le_bytes(*positions_len);
        let mut positions = Vec::with_capacity(positions_len as usize + 1);

        let mut offset = 0;
        for _ in 0..positions_len {
            let positions_flat = array_ref![data_flat, offset, LIQUIDITY_POSITION_SIZE];
            #[allow(clippy::ptr_offset_with_cast)]
            let (
                pool,
                liquidity_amount,
                rewards_owed,
                rewards_estimated,
                cumulative_interest,
                last_update_ts,
                next_claim_ts,
            ) = array_refs![positions_flat, PUBKEY_BYTES, 8, 8, 8, 8, 8, 8];
            positions.push(LiquidityPosition {
                pool: Pubkey::new(pool),
                liquidity_amount: u64::from_le_bytes(*liquidity_amount),
                rewards_owed: u64::from_le_bytes(*rewards_owed),
                rewards_estimated: u64::from_le_bytes(*rewards_estimated),
                cumulative_interest: u64::from_le_bytes(*cumulative_interest),
                last_update_ts: i64::from_le_bytes(*last_update_ts),
                next_claim_ts: i64::from_le_bytes(*next_claim_ts),
            });
            offset += LIQUIDITY_POSITION_SIZE;
        }
        Ok(Self {
            is_initialized,
            owner: Pubkey::new(owner),
            positions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{math::*, solana_program::clock::Clock};
    use proptest::prelude::*;

    const REFRESH_PERIOD: i64 = 3600;
    const REFRESH_TIMES: i64 = 720;

    prop_compose! {
        fn liquidity_amount_and_ratio()(amount in 0..=u32::MAX)(
            liquidity_amount in Just(amount as u64 * 1_000_000u64),
            rewards_rate in 1_000..=10_000u64, // 0.01 ~ 0.1%
            period_number in 1i64..=10i64
        ) -> (u64, u64, i64) {
            (liquidity_amount, rewards_rate, period_number)
        }
    }

    proptest! {
        #[test]
        fn test_update_rewards(
            (liquidity_amount, rewards_rate, period_number) in liquidity_amount_and_ratio()
        ) {
            let mut liquidity_position = LiquidityPosition {
                liquidity_amount,
                ..Default::default()
            };
            liquidity_position.next_claim_ts += MIN_CLAIM_PERIOD;

            let exact_rate = WAD as u128 / rewards_rate as u128;
            let max_period_amount = liquidity_amount / rewards_rate;
            let min_period_amount = max_period_amount - max_period_amount / 1_000;

            for i in 1..=REFRESH_TIMES * period_number {
                liquidity_position
                    .calc_and_update_rewards(
                        Decimal::from_scaled_val(exact_rate),
                        i * REFRESH_PERIOD,
                    )
                    .unwrap();
                assert!(liquidity_position.rewards_estimated < max_period_amount);
            }
            assert!(liquidity_position.rewards_owed <= max_period_amount * period_number as u64);
            // 0.01% confidence
            assert!(liquidity_position.rewards_owed > min_period_amount * period_number as u64);
            assert_eq!(liquidity_position.next_claim_ts, MIN_CLAIM_PERIOD * (period_number + 1));
        }
    }

    #[test]
    fn test_failures() {
        let mut position = LiquidityPosition {
            liquidity_amount: u64::MAX,
            ..Default::default()
        };

        assert_eq!(
            position.deposit(100),
            Err(SwapError::CalculationFailure.into())
        );

        position.liquidity_amount = 100;
        assert_eq!(
            position.withdraw(200),
            Err(SwapError::InsufficientLiquidity.into())
        );

        position.liquidity_amount = 100;
        position.next_claim_ts = i64::MAX;
        assert_eq!(
            position.update_claim_ts(),
            Err(SwapError::CalculationFailure.into())
        );

        assert_eq!(
            position.claim_rewards(),
            Err(SwapError::InsufficientClaimAmount.into())
        );

        position.cumulative_interest = u64::MAX;
        position.rewards_owed = 100;
        assert_eq!(
            position.claim_rewards(),
            Err(SwapError::CalculationFailure.into())
        );
    }

    #[test]
    fn test_liquidity_provider_packing() {
        let is_initialized = true;
        let owner_key_raw = [1u8; 32];
        let owner = Pubkey::new_from_array(owner_key_raw);

        let pool_1_key_raw = [2u8; 32];
        let pool_1 = Pubkey::new_from_array(pool_1_key_raw);
        let liquidity_amount_1: u64 = 300;
        let rewards_owed_1: u64 = 100;
        let rewards_estimated_1: u64 = 40;
        let cumulative_interest_1: u64 = 1000;
        let last_update_ts_1 = Clock::clone(&Default::default()).unix_timestamp;
        let next_claim_ts_1 = last_update_ts_1 + MIN_CLAIM_PERIOD;

        let position_1 = LiquidityPosition {
            pool: pool_1,
            liquidity_amount: liquidity_amount_1,
            rewards_owed: rewards_owed_1,
            rewards_estimated: rewards_estimated_1,
            cumulative_interest: cumulative_interest_1,
            last_update_ts: last_update_ts_1,
            next_claim_ts: next_claim_ts_1,
        };

        let pool_2_key_raw = [3u8; 32];
        let pool_2 = Pubkey::new_from_array(pool_2_key_raw);
        let liquidity_amount_2: u64 = 500;
        let rewards_owed_2: u64 = 200;
        let rewards_estimated_2: u64 = 80;
        let cumulative_interest_2: u64 = 2000;
        let last_update_ts_2 = Clock::clone(&Default::default()).unix_timestamp + 300;
        let next_claim_ts_2 = last_update_ts_2 + MIN_CLAIM_PERIOD;

        let position_2 = LiquidityPosition {
            pool: pool_2,
            liquidity_amount: liquidity_amount_2,
            rewards_owed: rewards_owed_2,
            rewards_estimated: rewards_estimated_2,
            cumulative_interest: cumulative_interest_2,
            last_update_ts: last_update_ts_2,
            next_claim_ts: next_claim_ts_2,
        };

        let liquidity_provider = LiquidityProvider {
            is_initialized,
            owner,
            positions: vec![position_1, position_2],
        };

        let mut packed = [0u8; LiquidityProvider::LEN];
        LiquidityProvider::pack_into_slice(&liquidity_provider, &mut packed);
        let unpacked = LiquidityProvider::unpack(&packed).unwrap();
        assert_eq!(liquidity_provider, unpacked);

        let mut packed: Vec<u8> = vec![1];
        packed.extend_from_slice(&owner_key_raw);
        packed.extend_from_slice(&(2u8).to_le_bytes());
        packed.extend_from_slice(&pool_1_key_raw);
        packed.extend_from_slice(&liquidity_amount_1.to_le_bytes());
        packed.extend_from_slice(&rewards_owed_1.to_le_bytes());
        packed.extend_from_slice(&rewards_estimated_1.to_le_bytes());
        packed.extend_from_slice(&cumulative_interest_1.to_le_bytes());
        packed.extend_from_slice(&last_update_ts_1.to_le_bytes());
        packed.extend_from_slice(&next_claim_ts_1.to_le_bytes());
        packed.extend_from_slice(&pool_2_key_raw);
        packed.extend_from_slice(&liquidity_amount_2.to_le_bytes());
        packed.extend_from_slice(&rewards_owed_2.to_le_bytes());
        packed.extend_from_slice(&rewards_estimated_2.to_le_bytes());
        packed.extend_from_slice(&cumulative_interest_2.to_le_bytes());
        packed.extend_from_slice(&last_update_ts_2.to_le_bytes());
        packed.extend_from_slice(&next_claim_ts_2.to_le_bytes());

        packed.extend_from_slice(&[0u8; (MAX_LIQUIDITY_POSITIONS - 2) * LIQUIDITY_POSITION_SIZE]);

        let unpacked = LiquidityProvider::unpack(&packed).unwrap();
        assert_eq!(liquidity_provider, unpacked);

        let packed = [0u8; LiquidityProvider::LEN];
        let liquidity_provider: LiquidityProvider = Default::default();
        let unpack_unchecked = LiquidityProvider::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, liquidity_provider);
        let err = LiquidityProvider::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }
}
