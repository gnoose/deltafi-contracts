//! State transition types

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::UnixTimestamp,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

use crate::{curve::PMMState, error::SwapError, fees::Fees, math::*, rewards::Rewards};

use std::convert::TryFrom;

/// Current version of the program and all new accounts created
pub const PROGRAM_VERSION: u8 = 1;

/// Accounts are created with data zeroed out, so uninitialized state instances
/// will have the version set to 0.
pub const UNINITIALIZED_VERSION: u8 = 0;

/// Dex Default Configuration information
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigInfo {
    /// Version of DELTAFI
    pub version: u8,

    /// Bump seed for derived authority address
    /// Especially for deltafi mint
    pub bump_seed: u8,

    /// Deadline to transfer admin control to future_admin_key
    pub future_admin_deadline: UnixTimestamp,
    /// Public key of the admin account to be applied
    pub future_admin_key: Pubkey,
    /// Public key of admin account to execute admin instructions
    pub admin_key: Pubkey,

    /// Governance token mint
    pub deltafi_mint: Pubkey,

    /// Fees
    pub fees: Fees,
    /// Rewards
    pub rewards: Rewards,
}

impl Sealed for ConfigInfo {}
impl IsInitialized for ConfigInfo {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_VERSION
    }
}

#[doc(hidden)]
pub const CONFIG_INFO_SIZE: usize = 210;
impl Pack for ConfigInfo {
    const LEN: usize = CONFIG_INFO_SIZE;
    #[doc(hidden)]
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, CONFIG_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            bump_seed,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            deltafi_mint,
            fees,
            rewards,
        ) = array_refs![
            src,
            1,
            1,
            8,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN
        ];

        let version = u8::from_le_bytes(*version);
        if version > PROGRAM_VERSION {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            version,
            bump_seed: u8::from_le_bytes(*bump_seed),
            future_admin_deadline: i64::from_le_bytes(*future_admin_deadline),
            future_admin_key: Pubkey::new_from_array(*future_admin_key),
            admin_key: Pubkey::new_from_array(*admin_key),
            deltafi_mint: Pubkey::new_from_array(*deltafi_mint),
            fees: Fees::unpack_from_slice(fees)?,
            rewards: Rewards::unpack_from_slice(rewards)?,
        })
    }
    #[doc(hidden)]
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, CONFIG_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            version,
            bump_seed,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            deltafi_mint,
            fees,
            rewards,
        ) = mut_array_refs![
            dst,
            1,
            1,
            8,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN
        ];
        *version = self.version.to_le_bytes();
        *bump_seed = self.bump_seed.to_le_bytes();
        *future_admin_deadline = self.future_admin_deadline.to_le_bytes();
        future_admin_key.copy_from_slice(self.future_admin_key.as_ref());
        admin_key.copy_from_slice(self.admin_key.as_ref());
        deltafi_mint.copy_from_slice(self.deltafi_mint.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        self.rewards.pack_into_slice(&mut rewards[..]);
    }
}

/// Swap states.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwapInfo {
    /// Initialized state
    pub is_initialized: bool,

    /// Paused state
    pub is_paused: bool,

    /// Nonce used in program address
    /// The program address is created deterministically with the nonce,
    /// swap program id, and swap account pubkey.  This program address has
    /// authority over the swap's token A account, token B account, and pool
    /// token mint.
    pub nonce: u8,

    /// Token A
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,
    /// Mint information for token A
    pub token_a_mint: Pubkey,
    /// Mint information for token B
    pub token_b_mint: Pubkey,

    /// Public key of the admin token account to receive trading and / or withdrawal fees for token a
    pub admin_fee_key_a: Pubkey,
    /// Public key of the admin token account to receive trading and / or withdrawal fees for token b
    pub admin_fee_key_b: Pubkey,
    /// Fees
    pub fees: Fees,
    /// Rewards
    pub rewards: Rewards,

    /// PMM object
    pub pmm_state: PMMState,
    /// twap open flag
    pub is_open_twap: bool,
    /// block timestamp last - twap
    pub block_timestamp_last: u64,
    /// cumulative ticks in seconds
    pub cumulative_ticks: u64,
    /// base price cumulative last - twap
    pub base_price_cumulative_last: Decimal,
}

impl Sealed for SwapInfo {}
impl IsInitialized for SwapInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}
const SWAP_INFO_SIZE: usize = 461;
impl Pack for SwapInfo {
    const LEN: usize = SWAP_INFO_SIZE;

    /// Unpacks a byte buffer into a [SwapInfo](struct.SwapInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, SWAP_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            nonce,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            rewards,
            pmm_state,
            is_open_twap,
            block_timestamp_last,
            cumulative_ticks,
            base_price_cumulative_last,
        ) = array_refs![
            input,
            1,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN,
            PMMState::LEN,
            1,
            8,
            8,
            16
        ];
        Ok(Self {
            is_initialized: unpack_bool(is_initialized)?,
            is_paused: unpack_bool(is_paused)?,
            nonce: u8::from_le_bytes(*nonce),
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
            token_b_mint: Pubkey::new_from_array(*token_b_mint),
            admin_fee_key_a: Pubkey::new_from_array(*admin_fee_key_a),
            admin_fee_key_b: Pubkey::new_from_array(*admin_fee_key_b),
            fees: Fees::unpack_from_slice(fees)?,
            rewards: Rewards::unpack_from_slice(rewards)?,
            pmm_state: PMMState::unpack_from_slice(pmm_state)?,
            is_open_twap: unpack_bool(is_open_twap)?,
            block_timestamp_last: u64::from_le_bytes(*block_timestamp_last),
            cumulative_ticks: u64::from_le_bytes(*cumulative_ticks),
            base_price_cumulative_last: unpack_decimal(base_price_cumulative_last),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, SWAP_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            nonce,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            rewards,
            pmm_state,
            is_open_twap,
            block_timestamp_last,
            cumulative_ticks,
            base_price_cumulative_last,
        ) = mut_array_refs![
            output,
            1,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN,
            PMMState::LEN,
            1,
            8,
            8,
            16
        ];
        pack_bool(self.is_initialized, is_initialized);
        pack_bool(self.is_paused, is_paused);
        *nonce = self.nonce.to_le_bytes();
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
        token_b_mint.copy_from_slice(self.token_b_mint.as_ref());
        admin_fee_key_a.copy_from_slice(self.admin_fee_key_a.as_ref());
        admin_fee_key_b.copy_from_slice(self.admin_fee_key_b.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        self.rewards.pack_into_slice(&mut rewards[..]);
        self.pmm_state.pack_into_slice(&mut pmm_state[..]);
        pack_bool(self.is_open_twap, is_open_twap);
        *block_timestamp_last = self.block_timestamp_last.to_le_bytes();
        *cumulative_ticks = self.cumulative_ticks.to_le_bytes();
        pack_decimal(self.base_price_cumulative_last, base_price_cumulative_last);
    }
}

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

/// Pack decimal
pub fn pack_decimal(decimal: Decimal, dst: &mut [u8; 16]) {
    *dst = decimal
        .to_scaled_val()
        .expect("Decimal cannot be packed")
        .to_le_bytes();
}

/// Unpack decimal
pub fn unpack_decimal(src: &[u8; 16]) -> Decimal {
    Decimal::from_scaled_val(u128::from_le_bytes(*src))
}

/// Pack boolean
pub fn pack_bool(boolean: bool, dst: &mut [u8; 1]) {
    *dst = (boolean as u8).to_le_bytes()
}

/// Unpack boolean
pub fn unpack_bool(src: &[u8; 1]) -> Result<bool, ProgramError> {
    match u8::from_le_bytes(*src) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

#[cfg(feature = "test-bpf")]
mod tests {
    use super::*;
    use crate::{
        solana_program::clock::Clock,
        utils::{
            test_utils::{default_i, default_k, DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS},
            TWAP_OPENED,
        },
        v2curve::RState,
    };

    #[test]
    fn test_swap_info_packing() {
        let nonce = 255;
        let initial_amp_factor: u64 = 1;
        let target_amp_factor: u64 = 1;
        let start_ramp_ts: i64 = i64::MAX;
        let stop_ramp_ts: i64 = i64::MAX;
        let token_a_raw = [3u8; 32];
        let token_b_raw = [4u8; 32];
        let pool_mint_raw = [5u8; 32];
        let token_a_mint_raw = [6u8; 32];
        let token_b_mint_raw = [7u8; 32];
        let admin_fee_key_a_raw = [8u8; 32];
        let admin_fee_key_b_raw = [9u8; 32];
        let deltafi_token_raw = [10u8; 32];
        let deltafi_mint_raw = [11u8; 32];
        let token_a = Pubkey::new_from_array(token_a_raw);
        let token_b = Pubkey::new_from_array(token_b_raw);
        let deltafi_token = Pubkey::new_from_array(deltafi_token_raw);
        let pool_mint = Pubkey::new_from_array(pool_mint_raw);
        let token_a_mint = Pubkey::new_from_array(token_a_mint_raw);
        let token_b_mint = Pubkey::new_from_array(token_b_mint_raw);
        let deltafi_mint = Pubkey::new_from_array(deltafi_mint_raw);
        let admin_fee_key_a = Pubkey::new_from_array(admin_fee_key_a_raw);
        let admin_fee_key_b = Pubkey::new_from_array(admin_fee_key_b_raw);
        let admin_trade_fee_numerator = 1;
        let admin_trade_fee_denominator = 2;
        let admin_withdraw_fee_numerator = 3;
        let admin_withdraw_fee_denominator = 4;
        let trade_fee_numerator = 5;
        let trade_fee_denominator = 6;
        let withdraw_fee_numerator = 7;
        let withdraw_fee_denominator = 8;
        let fees = Fees {
            admin_trade_fee_numerator,
            admin_trade_fee_denominator,
            admin_withdraw_fee_numerator,
            admin_withdraw_fee_denominator,
            trade_fee_numerator,
            trade_fee_denominator,
            withdraw_fee_numerator,
            withdraw_fee_denominator,
        };
        let is_initialized = true;
        let is_paused = false;
        let trade_reward_numerator = 1;
        let trade_reward_denominator = 2;
        let trade_reward_cap = 100;
        let liquidity_reward_numerator = 1;
        let liquidity_reward_denominator = 1000;
        let rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
            liquidity_reward_numerator,
            liquidity_reward_denominator,
        };
        let k = default_k();
        let i = default_i();
        let r = RState::One;
        let base_target = FixedU64::zero();
        let quote_target = FixedU64::zero();
        let base_reserve = FixedU64::zero();
        let quote_reserve = FixedU64::zero();
        let pmm_state = PMMState::new(
            i,
            k,
            base_reserve,
            quote_reserve,
            base_target,
            quote_target,
            r,
        );
        let is_open_twap = TWAP_OPENED;
        let block_timestamp_last: i64 = Clock::clone(&Default::default()).unix_timestamp;
        let base_price_cumulative_last = FixedU64::zero();
        let receive_amount = FixedU64::zero();

        let swap_info = SwapInfo {
            is_initialized,
            is_paused,
            nonce,
            initial_amp_factor,
            target_amp_factor,
            start_ramp_ts,
            stop_ramp_ts,
            token_a,
            token_b,
            deltafi_token,
            pool_mint,
            token_a_mint,
            token_b_mint,
            deltafi_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            rewards,
            pmm_state,
            is_open_twap,
            block_timestamp_last,
            base_price_cumulative_last,
            receive_amount,
        };

        let mut packed = [0u8; SwapInfo::LEN];
        SwapInfo::pack_into_slice(&swap_info, &mut packed);
        let unpacked = SwapInfo::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let mut packed: Vec<u8> = vec![1, 0, nonce];
        packed.extend_from_slice(&initial_amp_factor.to_le_bytes());
        packed.extend_from_slice(&target_amp_factor.to_le_bytes());
        packed.extend_from_slice(&start_ramp_ts.to_le_bytes());
        packed.extend_from_slice(&stop_ramp_ts.to_le_bytes());
        packed.extend_from_slice(&token_a_raw);
        packed.extend_from_slice(&token_b_raw);
        packed.extend_from_slice(&deltafi_token_raw);
        packed.extend_from_slice(&pool_mint_raw);
        packed.extend_from_slice(&token_a_mint_raw);
        packed.extend_from_slice(&token_b_mint_raw);
        packed.extend_from_slice(&deltafi_mint_raw);
        packed.extend_from_slice(&admin_fee_key_a_raw);
        packed.extend_from_slice(&admin_fee_key_b_raw);
        packed.extend_from_slice(&admin_trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&admin_trade_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&admin_withdraw_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&admin_withdraw_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&trade_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&withdraw_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&withdraw_fee_denominator.to_le_bytes());

        let mut packed_rewards = [0u8; Rewards::LEN];
        rewards.pack_into_slice(&mut packed_rewards);
        packed.extend_from_slice(&packed_rewards);
        let mut packed_pmm_state = [0u8; PMMState::LEN];
        pmm_state.pack_into_slice(&mut packed_pmm_state);
        packed.extend_from_slice(&packed_pmm_state);
        packed.extend_from_slice(&is_open_twap.to_le_bytes());
        packed.extend_from_slice(&block_timestamp_last.to_le_bytes());
        let mut packed_base_price_cumulative_last = [0u8; FixedU64::LEN];
        base_price_cumulative_last.pack_into_slice(&mut packed_base_price_cumulative_last);
        packed.extend_from_slice(&packed_base_price_cumulative_last);
        let mut packed_receive_amount = [0u8; FixedU64::LEN];
        receive_amount.pack_into_slice(&mut packed_receive_amount);
        packed.extend_from_slice(&packed_receive_amount);

        let unpacked = SwapInfo::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let packed = [0u8; SwapInfo::LEN];
        let swap_info: SwapInfo = Default::default();
        let unpack_unchecked = SwapInfo::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, swap_info);
        let err = SwapInfo::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }

    #[test]
    fn test_config_info_packing() {
        let is_initialized = true;
        let is_paused = false;
        let amp_factor: u64 = 1;
        let future_admin_deadline: i64 = i64::MAX;
        let future_admin_key_raw = [1u8; 32];
        let admin_key_raw = [2u8; 32];
        let deltafi_mint_raw = [3u8; 32];
        let future_admin_key = Pubkey::new_from_array(future_admin_key_raw);
        let admin_key = Pubkey::new_from_array(admin_key_raw);
        let deltafi_mint = Pubkey::new_from_array(deltafi_mint_raw);
        let fees = DEFAULT_TEST_FEES;
        let rewards = DEFAULT_TEST_REWARDS;

        let config_info = ConfigInfo {
            is_initialized,
            is_paused,
            amp_factor,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            deltafi_mint,
            fees,
            rewards,
        };

        let mut packed = [0u8; ConfigInfo::LEN];
        ConfigInfo::pack_into_slice(&config_info, &mut packed);
        let unpacked = ConfigInfo::unpack(&packed).unwrap();
        assert_eq!(config_info, unpacked);

        let mut packed: Vec<u8> = vec![1, 0];
        packed.extend_from_slice(&amp_factor.to_le_bytes());
        packed.extend_from_slice(&future_admin_deadline.to_le_bytes());
        packed.extend_from_slice(&future_admin_key_raw);
        packed.extend_from_slice(&admin_key_raw);
        packed.extend_from_slice(&deltafi_mint_raw);
        packed.extend_from_slice(&DEFAULT_TEST_FEES.admin_trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_FEES.admin_trade_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_FEES.admin_withdraw_fee_numerator.to_le_bytes());
        packed.extend_from_slice(
            &DEFAULT_TEST_FEES
                .admin_withdraw_fee_denominator
                .to_le_bytes(),
        );
        packed.extend_from_slice(&DEFAULT_TEST_FEES.trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_FEES.trade_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_FEES.withdraw_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_FEES.withdraw_fee_denominator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_REWARDS.trade_reward_numerator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_REWARDS.trade_reward_denominator.to_le_bytes());
        packed.extend_from_slice(&DEFAULT_TEST_REWARDS.trade_reward_cap.to_le_bytes());
        let unpacked = ConfigInfo::unpack(&packed).unwrap();
        assert_eq!(config_info, unpacked);

        let packed = [0u8; ConfigInfo::LEN];
        let swap_info: ConfigInfo = Default::default();
        let unpack_unchecked = ConfigInfo::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, swap_info);
        let err = ConfigInfo::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }
}
