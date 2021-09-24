//! State transition types

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use crate::{bn::FixedU256, fees::Fees, oracle::Oracle, v2curve::RState};

/// Program states.
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

    /// Deadline to transfer admin control to future_admin_key
    pub future_admin_deadline: i64,
    /// Public key of the admin account to be applied
    pub future_admin_key: Pubkey,
    /// Public key of admin account to execute admin instructions
    pub admin_key: Pubkey,

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

    /// Oracle
    pub oracle: Oracle,
    /// Slope value - 0 < k < 1
    pub k: FixedU256,
    /// Mid price
    pub i: FixedU256,
    /// r status
    pub r: RState,
    /// base target price
    pub base_target: FixedU256,
    /// quote target price
    pub quote_target: FixedU256,
    /// base reserve price
    pub base_reserve: FixedU256,
    /// quote reserve price
    pub quote_reserve: FixedU256,
}

impl Sealed for SwapInfo {}
impl IsInitialized for SwapInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for SwapInfo {
    const LEN: usize = 984;

    /// Unpacks a byte buffer into a [SwapInfo](struct.SwapInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 984];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            is_paused,
            nonce,
            initial_amp_factor,
            target_amp_factor,
            start_ramp_ts,
            stop_ramp_ts,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            oracle,
            k,
            i,
            r,
            base_target,
            quote_target,
            base_reserve,
            quote_reserve,
        ) = array_refs![
            input, 1, 1, 1, 8, 8, 8, 8, 8, 32, 32, 32, 32, 32, 32, 32, 32, 32, 64, 204, 64, 64, 1,
            64, 64, 64, 64
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
            future_admin_deadline: i64::from_le_bytes(*future_admin_deadline),
            future_admin_key: Pubkey::new_from_array(*future_admin_key),
            admin_key: Pubkey::new_from_array(*admin_key),
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
            token_b_mint: Pubkey::new_from_array(*token_b_mint),
            admin_fee_key_a: Pubkey::new_from_array(*admin_fee_key_a),
            admin_fee_key_b: Pubkey::new_from_array(*admin_fee_key_b),
            fees: Fees::unpack_from_slice(fees)?,
            oracle: Oracle::unpack_from_slice(oracle)?,
            k: FixedU256::unpack_from_slice(k)?,
            i: FixedU256::unpack_from_slice(i)?,
            r: RState::unpack(r)?,
            base_target: FixedU256::unpack_from_slice(base_target)?,
            quote_target: FixedU256::unpack_from_slice(quote_target)?,
            base_reserve: FixedU256::unpack_from_slice(base_reserve)?,
            quote_reserve: FixedU256::unpack_from_slice(quote_reserve)?,
        })
    }

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 984];
        let (
            is_initialized,
            is_paused,
            nonce,
            initial_amp_factor,
            target_amp_factor,
            start_ramp_ts,
            stop_ramp_ts,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            oracle,
            k,
            i,
            r,
            base_target,
            quote_target,
            base_reserve,
            quote_reserve,
        ) = mut_array_refs![
            output, 1, 1, 1, 8, 8, 8, 8, 8, 32, 32, 32, 32, 32, 32, 32, 32, 32, 64, 204, 64, 64, 1,
            64, 64, 64, 64
        ];
        is_initialized[0] = self.is_initialized as u8;
        is_paused[0] = self.is_paused as u8;
        nonce[0] = self.nonce;
        *initial_amp_factor = self.initial_amp_factor.to_le_bytes();
        *target_amp_factor = self.target_amp_factor.to_le_bytes();
        *start_ramp_ts = self.start_ramp_ts.to_le_bytes();
        *stop_ramp_ts = self.stop_ramp_ts.to_le_bytes();
        *future_admin_deadline = self.future_admin_deadline.to_le_bytes();
        future_admin_key.copy_from_slice(self.future_admin_key.as_ref());
        admin_key.copy_from_slice(self.admin_key.as_ref());
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
        token_b_mint.copy_from_slice(self.token_b_mint.as_ref());
        admin_fee_key_a.copy_from_slice(self.admin_fee_key_a.as_ref());
        admin_fee_key_b.copy_from_slice(self.admin_fee_key_b.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        self.oracle.pack_into_slice(&mut oracle[..]);
        self.k.pack_into_slice(&mut k[..]);
        self.i.pack_into_slice(&mut i[..]);
        *r = self.r.pack();
        self.base_target.pack_into_slice(&mut base_target[..]);
        self.quote_target.pack_into_slice(&mut quote_target[..]);
        self.base_reserve.pack_into_slice(&mut base_reserve[..]);
        self.quote_reserve.pack_into_slice(&mut quote_reserve[..]);
    }
}

pub struct FarmBaseInfo {
    pub is_initialized: bool,
    /// Total allocation points
    pub total_alloc_point: u64,
    pub reward_unit: u64,
}

impl Sealed for FarmBaseInfo {}
impl IsInitialized for FarmBaseInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmBaseInfo {
    /// !! must calc out right size after deciding all field.
    const LEN: usize = 395;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 395];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            total_alloc_point,
            reward_unit,
        ) = array_refs![input, 1, 8, 8];
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
        let output = array_mut_ref![output, 0, 395];
        let (
            is_initialized,
            total_alloc_point,
            reward_unit,
        ) = mut_array_refs![output, 1, 8, 8];
        is_initialized[0] = self.is_initialized as u8;
        *total_alloc_point = self.total_alloc_point.to_le_bytes();
        *reward_unit = self.reward_unit.to_le_bytes();
    }
}

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
    /// !! must calc out right size after deciding all field.
    const LEN: usize = 395;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 395];
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
        let output = array_mut_ref![output, 0, 395];
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

pub struct FarmingUserInfo {
    pub is_initialized: bool,
    pub amount: u64,
    pub reward_debt: u64,
    pub timestamp: i64,
    pub pending_deltafi: u64,
}

impl Sealed for FarmingUserInfo {}
impl IsInitialized for FarmingUserInfo {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for FarmingUserInfo {
    /// !! must calc out right size after deciding all field.
    const LEN: usize = 395;

    /// Unpacks a byte buffer into a [FarmInfo](struct.FarmInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 395];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            amount,
            reward_debt,
            timestamp,
            pending_deltafi,
        ) = array_refs![input, 1, 8, 8, 8, 8];
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
        let output = array_mut_ref![output, 0, 395];
        let (
            is_initialized,
            amount,
            reward_debt,
            timestamp,
            pending_deltafi,
        ) = mut_array_refs![output, 1, 8, 8, 8, 8];
        is_initialized[0] = self.is_initialized as u8;
        *amount = self.amount.to_le_bytes();
        *reward_debt = self.reward_debt.to_le_bytes();
        *timestamp = self.timestamp.to_le_bytes();
        *pending_deltafi = self.pending_deltafi.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::{default_i, default_k};

    #[test]
    fn test_swap_info_packing() {
        let nonce = 255;
        let initial_amp_factor: u64 = 1;
        let target_amp_factor: u64 = 1;
        let start_ramp_ts: i64 = i64::MAX;
        let stop_ramp_ts: i64 = i64::MAX;
        let future_admin_deadline: i64 = i64::MAX;
        let future_admin_key_raw = [1u8; 32];
        let admin_key_raw = [2u8; 32];
        let token_a_raw = [3u8; 32];
        let token_b_raw = [4u8; 32];
        let pool_mint_raw = [5u8; 32];
        let token_a_mint_raw = [6u8; 32];
        let token_b_mint_raw = [7u8; 32];
        let admin_fee_key_a_raw = [8u8; 32];
        let admin_fee_key_b_raw = [9u8; 32];
        let admin_key = Pubkey::new_from_array(admin_key_raw);
        let future_admin_key = Pubkey::new_from_array(future_admin_key_raw);
        let token_a = Pubkey::new_from_array(token_a_raw);
        let token_b = Pubkey::new_from_array(token_b_raw);
        let pool_mint = Pubkey::new_from_array(pool_mint_raw);
        let token_a_mint = Pubkey::new_from_array(token_a_mint_raw);
        let token_b_mint = Pubkey::new_from_array(token_b_mint_raw);
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
        let oracle = Oracle::new(token_a, token_b);
        let k = default_k();
        let i = default_i();
        let r = RState::One;
        let base_target = FixedU256::zero();
        let quote_target = FixedU256::zero();
        let base_reserve = FixedU256::zero();
        let quote_reserve = FixedU256::zero();

        let is_initialized = true;
        let is_paused = false;
        let swap_info = SwapInfo {
            is_initialized,
            is_paused,
            nonce,
            initial_amp_factor,
            target_amp_factor,
            start_ramp_ts,
            stop_ramp_ts,
            future_admin_deadline,
            future_admin_key,
            admin_key,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            admin_fee_key_a,
            admin_fee_key_b,
            fees,
            oracle,
            k,
            i,
            r,
            base_target,
            quote_target,
            base_reserve,
            quote_reserve,
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
        packed.extend_from_slice(&future_admin_deadline.to_le_bytes());
        packed.extend_from_slice(&future_admin_key_raw);
        packed.extend_from_slice(&admin_key_raw);
        packed.extend_from_slice(&token_a_raw);
        packed.extend_from_slice(&token_b_raw);
        packed.extend_from_slice(&pool_mint_raw);
        packed.extend_from_slice(&token_a_mint_raw);
        packed.extend_from_slice(&token_b_mint_raw);
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
        let mut packed_oracle = [0u8; Oracle::LEN];
        oracle.pack_into_slice(&mut packed_oracle);
        packed.extend_from_slice(&packed_oracle);
        let mut packed_k = [0u8; FixedU256::LEN];
        k.pack_into_slice(&mut packed_k);
        packed.extend_from_slice(&packed_k);
        let mut packed_i = [0u8; FixedU256::LEN];
        i.pack_into_slice(&mut packed_i);
        packed.extend_from_slice(&packed_i);
        let packed_r = r.pack();
        packed.extend_from_slice(&packed_r);

        let mut packed_base_target = [0u8; FixedU256::LEN];
        base_target.pack_into_slice(&mut packed_base_target);
        packed.extend_from_slice(&packed_base_target);
        let mut packed_quote_target = [0u8; FixedU256::LEN];
        quote_target.pack_into_slice(&mut packed_quote_target);
        packed.extend_from_slice(&packed_quote_target);
        let mut packed_base_reserve = [0u8; FixedU256::LEN];
        base_reserve.pack_into_slice(&mut packed_base_reserve);
        packed.extend_from_slice(&packed_base_reserve);
        let mut packed_quote_reserve = [0u8; FixedU256::LEN];
        quote_reserve.pack_into_slice(&mut packed_quote_reserve);
        packed.extend_from_slice(&packed_quote_reserve);

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
