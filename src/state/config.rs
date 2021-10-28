use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

use super::*;

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
pub const CONFIG_INFO_SIZE: usize = 170;
impl Pack for ConfigInfo {
    const LEN: usize = CONFIG_INFO_SIZE;
    #[doc(hidden)]
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, CONFIG_INFO_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (version, bump_seed, admin_key, deltafi_mint, fees, rewards) = array_refs![
            src,
            1,
            1,
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
        let (version, bump_seed, admin_key, deltafi_mint, fees, rewards) = mut_array_refs![
            dst,
            1,
            1,
            PUBKEY_BYTES,
            PUBKEY_BYTES,
            Fees::LEN,
            Rewards::LEN
        ];
        *version = self.version.to_le_bytes();
        *bump_seed = self.bump_seed.to_le_bytes();
        admin_key.copy_from_slice(self.admin_key.as_ref());
        deltafi_mint.copy_from_slice(self.deltafi_mint.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
        self.rewards.pack_into_slice(&mut rewards[..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_info_packing() {
        let version = PROGRAM_VERSION;
        let bump_seed = 255;
        let admin_key_raw = [2u8; 32];
        let deltafi_mint_raw = [3u8; 32];
        let admin_key = Pubkey::new_from_array(admin_key_raw);
        let deltafi_mint = Pubkey::new_from_array(deltafi_mint_raw);
        let fees = DEFAULT_TEST_FEES;
        let rewards = DEFAULT_TEST_REWARDS;

        let config_info = ConfigInfo {
            version,
            bump_seed,
            admin_key,
            deltafi_mint,
            fees,
            rewards,
        };

        let mut packed = [0u8; ConfigInfo::LEN];
        ConfigInfo::pack_into_slice(&config_info, &mut packed);
        let unpacked = ConfigInfo::unpack(&packed).unwrap();
        assert_eq!(config_info, unpacked);

        let mut packed: Vec<u8> = vec![PROGRAM_VERSION];
        packed.extend_from_slice(&bump_seed.to_le_bytes());
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
        packed.extend_from_slice(
            &DEFAULT_TEST_REWARDS
                .liquidity_reward_numerator
                .to_le_bytes(),
        );
        packed.extend_from_slice(
            &DEFAULT_TEST_REWARDS
                .liquidity_reward_denominator
                .to_le_bytes(),
        );
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
