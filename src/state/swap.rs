use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

use super::*;
use crate::{curve::PMMState, math::*};

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
}
