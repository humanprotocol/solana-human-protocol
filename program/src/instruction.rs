//! Instruction types
#![allow(clippy::too_many_arguments)]

use crate::state::{DataHash, DataUrl, URL_LEN};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar,
};
use std::{convert::TryInto, mem::size_of};
/// Instructions supported by the escrow program
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowInstruction {
    /// Initializes a new escrow.
    ///
    /// This instructions receives new uninitialized account and initializes
    /// new escrow on it. No signers is required, this instruction should called
    /// right after escrow account creation.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. [w] Account for the new escrow
    /// 1. [] Clock sysvar
    /// 2. [] Mint account for token managed by this escrow
    /// 3. [] Token account where escrow funds will be stored
    /// 4. [] Escrow launcher account
    /// 5. [] Escrow canceler account
    /// 6. [] Canceler's token account to receive escrow funds
    Initialize {
        /// Escrow duration in seconds, escrow can only be canceled after its duration expires
        duration: u64,
    },

    /// Setup initialized escrow and moves it into pending state.
    ///
    /// This instruction must be signed by one of the trusted handlers
    /// which are either launcher or canceler.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. [w] Escrow account
    /// 1. [s] Trusted handler signing this transaction
    /// 2. [] Clock sysvar
    /// 3. [] Signer account for the reputation oracle for this escrow
    /// 4. [] Reputation oracle's token account to receive fees
    /// 5. [] Signer account for the recording oracle for this escrow
    /// 6. [] Recording oracle's token account to receive fees
    Setup {
        /// Reputation oracle fee in percents
        reputation_oracle_stake: u8,

        /// Recording oracle fee in percents
        recording_oracle_stake: u8,

        /// Manifest URL
        manifest_url: DataUrl,

        /// Manifest hash
        manifest_hash: DataHash,
    },

    /// Store job results
    ///
    /// When the job is over save total amount of tokens, number of recepients and
    /// final results URL and hash. Must be signed by one of the trusted
    /// handlers.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. [w] Escrow account
    /// 1. [s] Trusted handler signing this transaction
    /// 2. [] Clock sysvar
    StoreResults {
        /// Total amount to pay
        total_amount: u64,

        /// Total number of recipients
        total_recipients: u64,

        /// Final results URL
        final_results_url: DataUrl,

        /// Final results hash
        final_results_hash: DataHash,
    },
    /// Do a single payout
    ///
    /// After results are stored send this message multiple times to send tokens
    /// to participants as well as oracle's fees. Must be signed by one of the trusted
    /// handlers.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. [w] Escrow account
    /// 1. [s] Trusted handler signing this transaction
    /// 2. [] Clock sysvar
    /// 3. [w] Escrow token sending account
    /// 4. [] Escrow signing authority (token sending account's owner)
    /// 5. [w] Payment recipient
    /// 6. [w] Reputation oracle's token account to receive fees
    /// 7. [w] Recording oracle's token account to receive fees
    /// 8. [] Token contract program
    Payout {
        /// Amount of tokens to pay
        amount: u64,
    },
    /// Cancel escrow
    ///
    /// Before escrow is finalized it is possible to cancel it and send all funds to
    /// the canceler token account. Must be signed by one of the trusted
    /// handlers.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. [w] Escrow account
    /// 1. [s] Trusted handler signing this transaction
    /// 2. [w] Escrow token sending account
    /// 3. [] Escrow signing authority (token sending account's owner)
    /// 4. [w] Canceler token account to receive funds
    /// 5. [] Token contract program
    Cancel,

    /// Complete escrow
    ///
    /// When payouts are complete it is possible to mark this escrow complete which
    /// simply changes its status. Must be signed by one of the trusted
    /// handlers.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. [w] Escrow account
    /// 1. [s] Trusted handler signing this transaction
    /// 2. [] Clock sysvar
    Complete,
}

impl EscrowInstruction {
    /// Unpacks a byte buffer into [EscrowInstruction](enum.EscrowInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;
        Ok(match tag {
            1 => {
                let (duration, _rest) = Self::unpack_u64(rest)?;
                Self::Initialize { duration }
            }
            2 => {
                let (reputation_oracle_stake, rest) = Self::unpack_u8(rest)?;
                let (recording_oracle_stake, rest) = Self::unpack_u8(rest)?;
                let (manifest_url, rest) = Self::unpack_url(rest)?;
                let (manifest_hash, _rest) = Self::unpack_hash(rest)?;
                Self::Setup {
                    reputation_oracle_stake,
                    recording_oracle_stake,
                    manifest_url,
                    manifest_hash,
                }
            }
            3 => {
                let (total_amount, rest) = Self::unpack_u64(rest)?;
                let (total_recipients, rest) = Self::unpack_u64(rest)?;
                let (final_results_url, rest) = Self::unpack_url(rest)?;
                let (final_results_hash, _rest) = Self::unpack_hash(rest)?;
                Self::StoreResults {
                    total_amount,
                    total_recipients,
                    final_results_url,
                    final_results_hash,
                }
            }
            4 => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                Self::Payout { amount }
            }
            5 => Self::Cancel,
            6 => Self::Complete,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }

    /// Packs a [EscrowInstruction](enum.EscrowInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize { duration } => {
                buf.push(1);
                buf.extend(&duration.to_le_bytes());
            }
            Self::Setup {
                reputation_oracle_stake,
                recording_oracle_stake,
                manifest_url,
                manifest_hash,
            } => {
                buf.push(2);
                buf.push(reputation_oracle_stake);
                buf.push(recording_oracle_stake);
                buf.extend(manifest_url.as_ref());
                buf.extend(manifest_hash.as_ref());
            }
            Self::StoreResults {
                total_amount,
                total_recipients,
                final_results_url,
                final_results_hash,
            } => {
                buf.push(3);
                buf.extend(&total_amount.to_le_bytes());
                buf.extend(&total_recipients.to_le_bytes());
                buf.extend(final_results_url.as_ref());
                buf.extend(final_results_hash.as_ref());
            }
            Self::Payout { amount } => {
                buf.push(4);
                buf.extend(&amount.to_le_bytes());
            }
            Self::Cancel => buf.push(5),
            Self::Complete => buf.push(6),
        }
        buf
    }

    fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
        match input.split_first() {
            Option::Some((value, rest)) => Ok((*value, rest)),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (bytes, rest) = input.split_at(8);
            Ok((
                u64::from_le_bytes(
                    bytes
                        .try_into()
                        .or(Err(ProgramError::InvalidInstructionData))?,
                ),
                rest,
            ))
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }

    fn unpack_hash(input: &[u8]) -> Result<(DataHash, &[u8]), ProgramError> {
        if input.len() >= 20 {
            let (bytes, rest) = input.split_at(20);
            Ok((
                DataHash::new_from_array(
                    bytes
                        .try_into()
                        .or(Err(ProgramError::InvalidInstructionData))?,
                ),
                rest,
            ))
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }

    fn unpack_url(input: &[u8]) -> Result<(DataUrl, &[u8]), ProgramError> {
        if input.len() >= URL_LEN {
            let (bytes, rest) = input.split_at(URL_LEN);
            let mut bytes_copy = [0u8; URL_LEN];
            bytes_copy.copy_from_slice(bytes);
            Ok((DataUrl::new_from_array(bytes_copy), rest))
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }
}

/// Creates `Initialize` instruction.
pub fn initialize(
    escrow_program_id: &Pubkey,
    escrow: &Pubkey,
    token_mint: &Pubkey,
    token_account: &Pubkey,
    launcher: &Pubkey,
    canceler: &Pubkey,
    canceler_token_account: &Pubkey,
    duration: u64,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::Initialize { duration }.pack();

    let accounts = vec![
        AccountMeta::new(*escrow, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*token_mint, false),
        AccountMeta::new_readonly(*token_account, false),
        AccountMeta::new_readonly(*launcher, false),
        AccountMeta::new_readonly(*canceler, false),
        AccountMeta::new_readonly(*canceler_token_account, false),
    ];

    Ok(Instruction {
        program_id: *escrow_program_id,
        accounts,
        data,
    })
}

/// Creates `Setup` instruction
pub fn setup(
    escrow_program_id: &Pubkey,
    escrow: &Pubkey,
    trusted_handler: &Pubkey,
    reputation_oracle: &Pubkey,
    reputation_oracle_token_account: &Pubkey,
    reputation_oracle_stake: u8,
    recording_oracle: &Pubkey,
    recording_oracle_token_account: &Pubkey,
    recording_oracle_stake: u8,
    manifest_url: &DataUrl,
    manifest_hash: &DataHash,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::Setup {
        reputation_oracle_stake,
        recording_oracle_stake,
        manifest_url: *manifest_url,
        manifest_hash: *manifest_hash,
    }
    .pack();

    let accounts = vec![
        AccountMeta::new(*escrow, false),
        AccountMeta::new_readonly(*trusted_handler, true),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*reputation_oracle, false),
        AccountMeta::new_readonly(*reputation_oracle_token_account, false),
        AccountMeta::new_readonly(*recording_oracle, false),
        AccountMeta::new_readonly(*recording_oracle_token_account, false),
    ];

    Ok(Instruction {
        program_id: *escrow_program_id,
        accounts,
        data,
    })
}

/// Creates `StoreResults` instruction
pub fn store_results(
    escrow_program_id: &Pubkey,
    escrow: &Pubkey,
    trusted_handler: &Pubkey,
    total_amount: u64,
    total_recipients: u64,
    final_results_url: &DataUrl,
    final_results_hash: &DataHash,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::StoreResults {
        total_amount,
        total_recipients,
        final_results_url: *final_results_url,
        final_results_hash: *final_results_hash,
    }
    .pack();

    let accounts = vec![
        AccountMeta::new(*escrow, false),
        AccountMeta::new_readonly(*trusted_handler, true),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *escrow_program_id,
        accounts,
        data,
    })
}

/// Creates `Payout` instruction
pub fn payout(
    escrow_program_id: &Pubkey,
    escrow: &Pubkey,
    trusted_handler: &Pubkey,
    escrow_token_account: &Pubkey,
    escrow_authority: &Pubkey,
    recipient_token_account: &Pubkey,
    reputation_oracle_token_account: &Pubkey,
    recording_oracle_token_account: &Pubkey,
    token_program_id: &Pubkey,
    amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::Payout { amount }.pack();

    let accounts = vec![
        AccountMeta::new(*escrow, false),
        AccountMeta::new_readonly(*trusted_handler, true),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new(*escrow_token_account, false),
        AccountMeta::new_readonly(*escrow_authority, false),
        AccountMeta::new(*recipient_token_account, false),
        AccountMeta::new(*reputation_oracle_token_account, false),
        AccountMeta::new(*recording_oracle_token_account, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *escrow_program_id,
        accounts,
        data,
    })
}

/// Creates `Cancel` instruction
pub fn cancel(
    escrow_program_id: &Pubkey,
    escrow: &Pubkey,
    trusted_handler: &Pubkey,
    escrow_token_account: &Pubkey,
    escrow_authority: &Pubkey,
    canceler_token_account: &Pubkey,
    token_program_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::Cancel.pack();

    let accounts = vec![
        AccountMeta::new(*escrow, false),
        AccountMeta::new_readonly(*trusted_handler, true),
        AccountMeta::new(*escrow_token_account, false),
        AccountMeta::new_readonly(*escrow_authority, false),
        AccountMeta::new(*canceler_token_account, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *escrow_program_id,
        accounts,
        data,
    })
}

/// Creates `Complete` instruction
pub fn complete(
    escrow_program_id: &Pubkey,
    escrow: &Pubkey,
    trusted_handler: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = EscrowInstruction::Complete.pack();

    let accounts = vec![
        AccountMeta::new(*escrow, false),
        AccountMeta::new_readonly(*trusted_handler, true),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];

    Ok(Instruction {
        program_id: *escrow_program_id,
        accounts,
        data,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_instruction_packing() {
        let check = EscrowInstruction::Initialize {
            duration: 2592000, // 0x0000000000278D00
        };
        let packed = check.pack();
        let expect: Vec<u8> = vec![1, 0x00, 0x8D, 0x27, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(packed, expect);
        let unpacked = EscrowInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let check = EscrowInstruction::Setup {
            reputation_oracle_stake: 5,
            recording_oracle_stake: 10,
            manifest_url: DataUrl::new_from_array([10; URL_LEN]),
            manifest_hash: DataHash::new_from_array([11; 20]),
        };
        let packed = check.pack();
        let mut expect: Vec<u8> = vec![2, 5, 10];
        expect.extend(&[10; URL_LEN]);
        expect.extend(&[11; 20]);
        assert_eq!(packed, expect);
        let unpacked = EscrowInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let check = EscrowInstruction::StoreResults {
            total_amount: 1000000,  // 0x00000000000F4240
            total_recipients: 1000, // 0x00000000000003E8
            final_results_url: DataUrl::new_from_array([21; URL_LEN]),
            final_results_hash: DataHash::new_from_array([22; 20]),
        };
        let packed = check.pack();
        let mut expect: Vec<u8> = vec![3];
        expect.extend(&[0x40, 0x42, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00]);
        expect.extend(&[0xE8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        expect.extend(&[21; URL_LEN]);
        expect.extend(&[22; 20]);
        assert_eq!(packed, expect);
        let unpacked = EscrowInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let check = EscrowInstruction::Payout {
            amount: 1000000000000, // 0x000000E8D4A51000
        };
        let packed = check.pack();
        let expect: Vec<u8> = vec![4, 0x00, 0x10, 0xA5, 0xD4, 0xE8, 0x00, 0x00, 0x00];
        assert_eq!(packed, expect);
        let unpacked = EscrowInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let check = EscrowInstruction::Cancel;
        let packed = check.pack();
        let expect: Vec<u8> = vec![5];
        assert_eq!(packed, expect);
        let unpacked = EscrowInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let check = EscrowInstruction::Complete;
        let packed = check.pack();
        let expect: Vec<u8> = vec![6];
        assert_eq!(packed, expect);
        let unpacked = EscrowInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }
}
