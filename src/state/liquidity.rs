use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::UnixTimestamp,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

use crate::error::SwapError;

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
    /// Create new provider
    pub fn new(owner: Pubkey, positions: Vec<LiquidityPosition>) -> Self {
        let mut provider = Self::default();
        provider.init(owner, positions);
        provider
    }

    /// Initialize a liquidity provider
    pub fn init(&mut self, owner: Pubkey, positions: Vec<LiquidityPosition>) {
        self.is_initialized = true;
        self.owner = owner;
        self.positions = positions;
    }

    /// Find position by pool
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

    fn find_position_index(&self, pool: Pubkey) -> Option<usize> {
        self.positions
            .iter()
            .position(|position| position.pool == pool)
    }

    /// Withdraw liquidity and remove it from deposits if zeroed out
    pub fn withdraw(&mut self, withdraw_amount: u64, position_index: usize) -> ProgramResult {
        let position = &mut self.positions[position_index];
        if withdraw_amount == position.liquidity_amount && position.rewards_owed == 0 {
            self.positions.remove(position_index);
        } else {
            position.withdraw(withdraw_amount)?;
        }
        Ok(())
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
    /// Create new liquidity
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
    pub fn deposit(&mut self, deposit_amount: u64) -> ProgramResult {
        self.liquidity_amount = self
            .liquidity_amount
            .checked_add(deposit_amount)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Withdraw liquidity
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
    pub fn update_claim_ts(&mut self) -> ProgramResult {
        if self.liquidity_amount == 0 {
            return Err(SwapError::LiquidityPositionEmpty.into());
        }
        self.next_claim_ts = self
            .next_claim_ts
            .checked_add(MIN_CLAIM_PERIOD)
            .ok_or(SwapError::CalculationFailure)?;
        Ok(())
    }

    /// Calculate and update rewards
    pub fn calc_and_update_rewards(
        &mut self,
        rewards_unit: u64,
        current_ts: UnixTimestamp,
    ) -> ProgramResult {
        let calc_period = current_ts
            .checked_sub(self.last_update_ts)
            .ok_or(SwapError::CalculationFailure)?;
        if calc_period > 0 {
            self.rewards_estimated = self
                .rewards_estimated
                .checked_add(
                    rewards_unit
                        .checked_mul(u64::try_from(calc_period).unwrap())
                        .ok_or(SwapError::CalculationFailure)?
                        .checked_div(u64::try_from(MIN_CLAIM_PERIOD).unwrap())
                        .ok_or(SwapError::CalculationFailure)?,
                )
                .ok_or(SwapError::CalculationFailure)?;
            self.last_update_ts = current_ts;
        }

        if current_ts.gt(&self.next_claim_ts) {
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
    pub fn claim_rewards(&mut self) -> ProgramResult {
        if self.rewards_owed == 0 {
            return Err(SwapError::InsufficientClaimAmount.into());
        }
        self.cumulative_interest = self
            .cumulative_interest
            .checked_add(self.rewards_owed)
            .ok_or(SwapError::CalculationFailure)?;
        self.rewards_owed = 0;
        Ok(())
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

        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };
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
