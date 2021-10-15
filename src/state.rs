//! State transition types

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use crate::{bn::FixedU64, fees::Fees, rewards::Rewards, v2curve::PMMState};

/// Dex Default Configuration information
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConfigInfo {
    /// Initialization state
    pub is_initialized: bool,

    /// Paused state
    pub is_paused: bool,

    /// Default amplification coefficient
    pub amp_factor: u64,

    /// Deadline to transfer admin control to future_admin_key
    pub future_admin_deadline: i64,
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
        self.is_initialized
    }
}
impl Pack for ConfigInfo {
    const LEN: usize = 202;
    #[doc(hidden)]
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, 202];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            amp_factor,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            deltafi_mint,
            fees,
            rewards,
        ) = array_refs![src, 1, 1, 8, 8, 32, 32, 32, 64, 24];

        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            is_paused: match is_paused {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            amp_factor: u64::from_le_bytes(*amp_factor),
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
        let dst = array_mut_ref![dst, 0, 202];
        let (
            is_initialized,
            is_paused,
            amp_factor,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            deltafi_mint,
            fees,
            rewards,
        ) = mut_array_refs![dst, 1, 1, 8, 8, 32, 32, 32, 64, 24];
        is_initialized[0] = self.is_initialized as u8;
        is_paused[0] = self.is_paused as u8;
        *amp_factor = self.amp_factor.to_le_bytes();
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

    /// Initial amplification coefficient (A)
    pub initial_amp_factor: u64,
    /// Target amplification coefficient (A)
    pub target_amp_factor: u64,
    /// Ramp A start timestamp
    pub start_ramp_ts: i64,
    /// Ramp A stop timestamp
    pub stop_ramp_ts: i64,

    /// Token A
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,
    /// Deltafi token
    pub deltafi_token: Pubkey,

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,
    /// Mint information for token A
    pub token_a_mint: Pubkey,
    /// Mint information for token B
    pub token_b_mint: Pubkey,
    /// deltafi tokens are rewarded when swapping is successfully processed.
    /// Mint deltafi token
    pub deltafi_mint: Pubkey,

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
    pub is_open_twap: u64,
    /// block timestamp last - twap
    pub block_timestamp_last: i64,
    /// base price cumulative last - twap
    pub base_price_cumulative_last: FixedU64,
    /// receive amount on swap
    pub receive_amount: FixedU64,
}

impl Sealed for SwapInfo {}
impl IsInitialized for SwapInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}
impl Pack for SwapInfo {
    const LEN: usize = 500;

    /// Unpacks a byte buffer into a [SwapInfo](struct.SwapInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 500];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
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
        ) = array_refs![
            input, 1, 1, 1, 8, 8, 8, 8, 32, 32, 32, 32, 32, 32, 32, 32, 32, 64, 24, 55, 8, 8, 9, 9
        ];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            is_paused: match is_paused {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            nonce: nonce[0],
            initial_amp_factor: u64::from_le_bytes(*initial_amp_factor),
            target_amp_factor: u64::from_le_bytes(*target_amp_factor),
            start_ramp_ts: i64::from_le_bytes(*start_ramp_ts),
            stop_ramp_ts: i64::from_le_bytes(*stop_ramp_ts),
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            deltafi_token: Pubkey::new_from_array(*deltafi_token),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
            token_b_mint: Pubkey::new_from_array(*token_b_mint),
            deltafi_mint: Pubkey::new_from_array(*deltafi_mint),
            admin_fee_key_a: Pubkey::new_from_array(*admin_fee_key_a),
            admin_fee_key_b: Pubkey::new_from_array(*admin_fee_key_b),
            fees: Fees::unpack_from_slice(fees)?,
            rewards: Rewards::unpack_from_slice(rewards)?,
            pmm_state: PMMState::unpack_from_slice(pmm_state)?,
            is_open_twap: u64::from_le_bytes(*is_open_twap),
            block_timestamp_last: i64::from_le_bytes(*block_timestamp_last),
            base_price_cumulative_last: FixedU64::unpack_from_slice(base_price_cumulative_last)?,
            receive_amount: FixedU64::unpack_from_slice(receive_amount)?,
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 500];
        let (
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
        ) = mut_array_refs![
            output, 1, 1, 1, 8, 8, 8, 8, 32, 32, 32, 32, 32, 32, 32, 32, 32, 64, 24, 55, 8, 8, 9, 9
        ];
        is_initialized[0] = self.is_initialized as u8;
        is_paused[0] = self.is_paused as u8;
        nonce[0] = self.nonce;
        *initial_amp_factor = self.initial_amp_factor.to_le_bytes();
        *target_amp_factor = self.target_amp_factor.to_le_bytes();
        *start_ramp_ts = self.start_ramp_ts.to_le_bytes();
        *stop_ramp_ts = self.stop_ramp_ts.to_le_bytes();
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        deltafi_token.copy_from_slice(self.deltafi_token.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
        token_b_mint.copy_from_slice(self.token_b_mint.as_ref());
        deltafi_mint.copy_from_slice(self.deltafi_mint.as_ref());
        admin_fee_key_a.copy_from_slice(self.admin_fee_key_a.as_ref());
        admin_fee_key_b.copy_from_slice(self.admin_fee_key_b.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        self.rewards.pack_into_slice(&mut rewards[..]);
        self.pmm_state.pack_into_slice(&mut pmm_state[..]);
        *is_open_twap = self.is_open_twap.to_le_bytes();
        *block_timestamp_last = self.block_timestamp_last.to_le_bytes();
        self.base_price_cumulative_last
            .pack_into_slice(&mut base_price_cumulative_last[..]);
        self.receive_amount.pack_into_slice(&mut receive_amount[..]);
    }
}

/// Farm's base information
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FarmBaseInfo {
    /// whether initialized
    pub is_initialized: bool,
    /// Total allocation points
    pub total_alloc_point: u64,
    /// reward unit
    pub reward_unit: u64,
}

impl Sealed for FarmBaseInfo {}
impl IsInitialized for FarmBaseInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmBaseInfo {
    const LEN: usize = 17;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 17];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, total_alloc_point, reward_unit) = array_refs![input, 1, 8, 8];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            total_alloc_point: u64::from_le_bytes(*total_alloc_point),
            reward_unit: u64::from_le_bytes(*reward_unit),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 17];
        let (is_initialized, total_alloc_point, reward_unit) = mut_array_refs![output, 1, 8, 8];
        is_initialized[0] = self.is_initialized as u8;
        *total_alloc_point = self.total_alloc_point.to_le_bytes();
        *reward_unit = self.reward_unit.to_le_bytes();
    }
}

/// Farm state
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FarmInfo {
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

    /// Deadline to transfer admin control to future_admin_key
    pub future_admin_deadline: i64,
    /// Public key of the admin account to be applied
    pub future_admin_key: Pubkey,
    /// Public key of admin account to execute admin instructions
    pub admin_key: Pubkey,

    /// Mint information for lp token
    pub pool_mint: Pubkey,
    /// Mint information for deltafi
    pub token_deltafi_mint: Pubkey,
    /// Fees
    pub fees: Fees,
    /// the value corresponding accDeltafiPerShare parameter to use in farming
    pub acc_deltafi_per_share: u64,
    /// Timestamp when calculate reward last     
    pub last_reward_timestamp: i64,
    /// allocation point
    pub alloc_point: u64,
}
impl Sealed for FarmInfo {}
impl IsInitialized for FarmInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmInfo {
    const LEN: usize = 227;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 227];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            nonce,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            pool_mint,
            token_deltafi_mint,
            fees,
            acc_deltafi_per_share,
            last_reward_timestamp,
            alloc_point,
        ) = array_refs![input, 1, 1, 1, 8, 32, 32, 32, 32, 64, 8, 8, 8];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            is_paused: match is_paused {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            nonce: nonce[0],
            future_admin_deadline: i64::from_le_bytes(*future_admin_deadline),
            future_admin_key: Pubkey::new_from_array(*future_admin_key),
            admin_key: Pubkey::new_from_array(*admin_key),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_deltafi_mint: Pubkey::new_from_array(*token_deltafi_mint),
            fees: Fees::unpack_from_slice(fees)?,
            acc_deltafi_per_share: u64::from_le_bytes(*acc_deltafi_per_share),
            last_reward_timestamp: i64::from_le_bytes(*last_reward_timestamp),
            alloc_point: u64::from_le_bytes(*alloc_point),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 227];
        let (
            is_initialized,
            is_paused,
            nonce,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            pool_mint,
            token_deltafi_mint,
            fees,
            acc_deltafi_per_share,
            last_reward_timestamp,
            alloc_point,
        ) = mut_array_refs![output, 1, 1, 1, 8, 32, 32, 32, 32, 64, 8, 8, 8];
        is_initialized[0] = self.is_initialized as u8;
        is_paused[0] = self.is_paused as u8;
        nonce[0] = self.nonce;
        *future_admin_deadline = self.future_admin_deadline.to_le_bytes();
        future_admin_key.copy_from_slice(self.future_admin_key.as_ref());
        admin_key.copy_from_slice(self.admin_key.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_deltafi_mint.copy_from_slice(self.token_deltafi_mint.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        *acc_deltafi_per_share = self.acc_deltafi_per_share.to_le_bytes();
        *last_reward_timestamp = self.last_reward_timestamp.to_le_bytes();
        *alloc_point = self.alloc_point.to_le_bytes();
    }
}

/// User's farming information
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FarmingUserInfo {
    /// whether initialized
    pub is_initialized: bool,
    /// user's amount
    pub amount: u64,
    /// user's reward debt
    pub reward_debt: u64,
    /// last update timestamp
    pub timestamp: i64,
    /// pending deltafi
    pub pending_deltafi: u64,
}

impl Sealed for FarmingUserInfo {}
impl IsInitialized for FarmingUserInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmingUserInfo {
    const LEN: usize = 33;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 33];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, amount, reward_debt, timestamp, pending_deltafi) =
            array_refs![input, 1, 8, 8, 8, 8];

        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            amount: u64::from_le_bytes(*amount),
            reward_debt: u64::from_le_bytes(*reward_debt),
            timestamp: i64::from_le_bytes(*timestamp),
            pending_deltafi: u64::from_le_bytes(*pending_deltafi),
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 33];
        let (is_initialized, amount, reward_debt, timestamp, pending_deltafi) =
            mut_array_refs![output, 1, 8, 8, 8, 8];
        is_initialized[0] = self.is_initialized as u8;
        *amount = self.amount.to_le_bytes();
        *reward_debt = self.reward_debt.to_le_bytes();
        *timestamp = self.timestamp.to_le_bytes();
        *pending_deltafi = self.pending_deltafi.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{
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
        let rewards = Rewards {
            trade_reward_numerator,
            trade_reward_denominator,
            trade_reward_cap,
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
        let block_timestamp_last: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
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
        SwapInfo::pack(swap_info, &mut packed).unwrap();
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
