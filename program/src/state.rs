//! State types

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use num_enum::TryFromPrimitive;
use solana_program::{
    clock::UnixTimestamp,
    program_error::ProgramError,
    program_option::COption,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};
use std::{fmt, mem, str::FromStr};

/// Size for the URL field
pub const URL_LEN: usize = 256;

/// Uninitialized Factory version
pub const UNINITIALIZED_FACTORY_VERSION: u8 = 0;

/// Factory account
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Factory {
    /// Factory's version
    pub version: u8,
}

impl Sealed for Factory {}
impl IsInitialized for Factory {
    fn is_initialized(&self) -> bool {
        self.version != UNINITIALIZED_FACTORY_VERSION
    }
}

impl Pack for Factory {
    const LEN: usize = mem::size_of::<Self>();

    /// Packs a [Factory](struct.Factory.html) into a byte buffer.
    fn pack_into_slice(&self, output: &mut [u8]) {
        output[0] = self.version;
    }

    /// Unpacks a byte buffer into a [Factory](struct.Factory.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        Ok(Factory { version: input[0] })
    }
}

/// Escrow state.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, TryFromPrimitive)]
pub enum EscrowState {
    /// Escrow is not yet initialized
    Uninitialized,
    /// Escrow is launched
    Launched,
    /// Escrow is pending payment
    Pending,
    /// Escrow is partially paid
    Partial,
    /// Escrow is fully paid
    Paid,
    /// Escrow is completed
    Complete,
    /// Escrow is cancelled, money returned
    Cancelled,
}

impl Default for EscrowState {
    fn default() -> Self {
        EscrowState::Uninitialized
    }
}

/// Stores job manifest hash
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DataHash([u8; 20]);

impl AsRef<[u8]> for DataHash {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl DataHash {
    /// Create new from fixed size array
    pub const fn new_from_array(data: [u8; 20]) -> Self {
        Self(data)
    }
    /// Create new from slice
    pub fn new_from_slice(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() != mem::size_of::<DataHash>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        let mut hash: Self = Default::default();
        hash.0.copy_from_slice(data);
        Ok(hash)
    }
}

/// Stores data URL
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct DataUrl([u8; URL_LEN]);

impl Default for DataUrl {
    fn default() -> Self {
        DataUrl([0; URL_LEN])
    }
}

impl fmt::Debug for DataUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0[..].fmt(f)
    }
}

impl PartialEq for DataUrl {
    fn eq(&self, other: &Self) -> bool {
        self.0.iter().zip(other.0.iter()).all(|(a, b)| a == b)
    }
}

impl AsRef<[u8]> for DataUrl {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl FromStr for DataUrl {
    type Err = ProgramError;

    fn from_str(s: &str) -> Result<Self, ProgramError> {
        let bytes = s.as_bytes();
        let length = bytes.len();
        if length > mem::size_of::<DataUrl>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let mut result: Self = Default::default();
        result.0[..length].copy_from_slice(bytes);
        Ok(result)
    }
}

impl DataUrl {
    /// Create new from fixed size array
    pub const fn new_from_array(data: [u8; URL_LEN]) -> Self {
        Self(data)
    }
}

/// Escrow data
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Escrow {
    /// Current state of escrow entity: Uninitialized, Launched, Pending, Partial, Paid, Complete, Cancelled
    pub state: EscrowState,
    /// Factory account this Escrow belongs to
    pub factory: Pubkey,
    /// Escrow expiration timestamp
    pub expires: UnixTimestamp,
    /// Program authority bump seed
    pub bump_seed: u8,
    /// Mint for the token handled by the escrow
    pub token_mint: Pubkey,
    /// Account to hold tokens for sendout, its owner should be escrow contract authority
    pub token_account: Pubkey,
    /// Pubkey of the reputation oracle
    pub reputation_oracle: COption<Pubkey>,
    /// Account for the reputation oracle to receive fee
    pub reputation_oracle_token_account: COption<Pubkey>,
    /// Reputation oracle fee (in percents)
    pub reputation_oracle_stake: u8,
    /// Pubkey of the recording oracle
    pub recording_oracle: COption<Pubkey>,
    /// Account for the recording oracle to receive fee
    pub recording_oracle_token_account: COption<Pubkey>,
    /// Recording oracle fee (in percents)
    pub recording_oracle_stake: u8,
    /// Launcher pubkey
    pub launcher: Pubkey,
    /// Canceler pubkey
    pub canceler: Pubkey,
    /// Account for the canceler to receive back tokens
    pub canceler_token_account: Pubkey,
    /// Total amount of tokens to pay out
    pub total_amount: u64,
    /// Total number of recepients
    pub total_recipients: u64,
    /// Amount in tokens already sent
    pub sent_amount: u64,
    /// Number of recepients already sent to
    pub sent_recipients: u64,
    /// Job manifest url
    pub manifest_url: DataUrl,
    /// Job manifest hash
    pub manifest_hash: DataHash,
    /// Job results url
    pub final_results_url: DataUrl,
    /// Job results hash
    pub final_results_hash: DataHash,
}

impl Sealed for Escrow {}
impl IsInitialized for Escrow {
    fn is_initialized(&self) -> bool {
        self.state != EscrowState::Uninitialized
    }
}

impl Pack for Escrow {
    const LEN: usize = 420 + URL_LEN + URL_LEN;

    /// Packs a [EscrowInfo](struct.EscrowInfo.html) into a byte buffer.
    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, Escrow::LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            expires_dst,
            bump_seed_dst,
            token_mint_dst,
            token_account_dst,
            reputation_oracle_dst,
            reputation_oracle_token_account_dst,
            reputation_oracle_stake_dst,
            recording_oracle_dst,
            recording_oracle_token_account_dst,
            recording_oracle_stake_dst,
            launcher_dst,
            canceler_dst,
            canceler_token_account_dst,
            total_amount_dst,
            total_recipients_dst,
            sent_amount_dst,
            sent_recipients_dst,
            state_dst,
            factory_acc,
            manifest_url_dst,
            manifest_hash_dst,
            final_results_url_dst,
            final_results_hash_dst,
        ) = mut_array_refs![
            output, 8, 1, 32, 32, 36, 36, 1, 36, 36, 1, 32, 32, 32, 8, 8, 8, 8, 1, 32, URL_LEN, 20,
            URL_LEN, 20
        ];
        expires_dst.copy_from_slice(&self.expires.to_le_bytes());
        bump_seed_dst[0] = self.bump_seed;
        token_mint_dst.copy_from_slice(self.token_mint.as_ref());
        token_account_dst.copy_from_slice(self.token_account.as_ref());
        pack_coption_key(&self.reputation_oracle, reputation_oracle_dst);
        pack_coption_key(
            &self.reputation_oracle_token_account,
            reputation_oracle_token_account_dst,
        );
        reputation_oracle_stake_dst[0] = self.reputation_oracle_stake;
        pack_coption_key(&self.recording_oracle, recording_oracle_dst);
        pack_coption_key(
            &self.recording_oracle_token_account,
            recording_oracle_token_account_dst,
        );
        recording_oracle_stake_dst[0] = self.recording_oracle_stake;
        launcher_dst.copy_from_slice(self.launcher.as_ref());
        canceler_dst.copy_from_slice(self.canceler.as_ref());
        canceler_token_account_dst.copy_from_slice(self.canceler_token_account.as_ref());
        total_amount_dst.copy_from_slice(&self.total_amount.to_le_bytes());
        total_recipients_dst.copy_from_slice(&self.total_recipients.to_le_bytes());
        sent_amount_dst.copy_from_slice(&self.sent_amount.to_le_bytes());
        sent_recipients_dst.copy_from_slice(&self.sent_recipients.to_le_bytes());
        state_dst[0] = self.state as u8;
        factory_acc.copy_from_slice(self.factory.as_ref());
        manifest_url_dst.copy_from_slice(self.manifest_url.as_ref());
        manifest_hash_dst.copy_from_slice(self.manifest_hash.as_ref());
        final_results_url_dst.copy_from_slice(self.final_results_url.as_ref());
        final_results_hash_dst.copy_from_slice(self.final_results_hash.as_ref());
    }

    /// Unpacks a byte buffer into a [EscrowInfo](struct.EscrowInfo.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, Escrow::LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            expires_src,
            bump_seed_src,
            token_mint_src,
            token_account_src,
            reputation_oracle_src,
            reputation_oracle_token_account_src,
            reputation_oracle_stake_src,
            recording_oracle_src,
            recording_oracle_token_account_src,
            recording_oracle_stake_src,
            launcher_src,
            canceler_src,
            canceler_token_account_src,
            total_amount_src,
            total_recipients_src,
            sent_amount_src,
            sent_recipients_src,
            state_src,
            factory_acc,
            manifest_url_src,
            manifest_hash_src,
            final_results_url_src,
            final_results_hash_src,
        ) = array_refs![
            input, 8, 1, 32, 32, 36, 36, 1, 36, 36, 1, 32, 32, 32, 8, 8, 8, 8, 1, 32, URL_LEN, 20,
            URL_LEN, 20
        ];
        Ok(Escrow {
            expires: UnixTimestamp::from_le_bytes(*expires_src),

            bump_seed: bump_seed_src[0],

            token_mint: Pubkey::new_from_array(*token_mint_src),
            token_account: Pubkey::new_from_array(*token_account_src),

            reputation_oracle: unpack_coption_key(reputation_oracle_src)?,
            reputation_oracle_token_account: unpack_coption_key(
                reputation_oracle_token_account_src,
            )?,
            reputation_oracle_stake: reputation_oracle_stake_src[0],

            recording_oracle: unpack_coption_key(recording_oracle_src)?,
            recording_oracle_token_account: unpack_coption_key(recording_oracle_token_account_src)?,
            recording_oracle_stake: recording_oracle_stake_src[0],

            launcher: Pubkey::new_from_array(*launcher_src),
            canceler: Pubkey::new_from_array(*canceler_src),
            canceler_token_account: Pubkey::new_from_array(*canceler_token_account_src),
            total_amount: u64::from_le_bytes(*total_amount_src),
            total_recipients: u64::from_le_bytes(*total_recipients_src),
            sent_amount: u64::from_le_bytes(*sent_amount_src),
            sent_recipients: u64::from_le_bytes(*sent_recipients_src),
            state: EscrowState::try_from_primitive(state_src[0])
                .or(Err(ProgramError::InvalidAccountData))?,

            factory: Pubkey::new_from_array(*factory_acc),

            manifest_url: DataUrl::new_from_array(*manifest_url_src),
            manifest_hash: DataHash::new_from_array(*manifest_hash_src),

            final_results_url: DataUrl::new_from_array(*final_results_url_src),
            final_results_hash: DataHash::new_from_array(*final_results_hash_src),
        })
    }
}

// Helpers
fn pack_coption_key(src: &COption<Pubkey>, dst: &mut [u8; 36]) {
    let (tag, body) = mut_array_refs![dst, 4, 32];
    match src {
        COption::Some(key) => {
            *tag = [1, 0, 0, 0];
            body.copy_from_slice(key.as_ref());
        }
        COption::None => {
            *tag = [0; 4];
        }
    }
}
fn unpack_coption_key(src: &[u8; 36]) -> Result<COption<Pubkey>, ProgramError> {
    let (tag, body) = array_refs![src, 4, 32];
    match *tag {
        [0, 0, 0, 0] => Ok(COption::None),
        [1, 0, 0, 0] => Ok(COption::Some(Pubkey::new_from_array(*body))),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use solana_program::{program_pack::Pack, pubkey::Pubkey};

    #[test]
    fn test_state_packing() {
        let obj = Escrow {
            expires: 1606402240,
            bump_seed: 250,
            state: EscrowState::Launched,
            factory: Pubkey::new_from_array([6; 32]),
            token_mint: Pubkey::new_from_array([1; 32]),
            token_account: Pubkey::new_from_array([2; 32]),
            reputation_oracle: COption::Some(Pubkey::new_from_array([3; 32])),
            reputation_oracle_token_account: COption::Some(Pubkey::new_from_array([4; 32])),
            reputation_oracle_stake: 5,
            recording_oracle: COption::None,
            recording_oracle_token_account: COption::Some(Pubkey::new_from_array([6; 32])),
            recording_oracle_stake: 10,
            launcher: Pubkey::new_from_array([7; 32]),
            canceler: Pubkey::new_from_array([8; 32]),
            canceler_token_account: Pubkey::new_from_array([9; 32]),
            total_amount: 20000000,
            total_recipients: 1000000,
            sent_amount: 2000000,
            sent_recipients: 100000,
            manifest_url: DataUrl::new_from_array([10; URL_LEN]),
            manifest_hash: DataHash::new_from_array([11; 20]),
            final_results_url: DataUrl::new_from_array([12; URL_LEN]),
            final_results_hash: DataHash::new_from_array([13; 20]),
        };
        let mut packed_obj: [u8; Escrow::LEN] = [0; Escrow::LEN];
        Escrow::pack(obj, &mut packed_obj).unwrap();
        let unpacked_obj = Escrow::unpack(&packed_obj).unwrap();
        assert_eq!(unpacked_obj, obj);
    }
}
