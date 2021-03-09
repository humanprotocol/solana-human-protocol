//! Error types

use num_derive::FromPrimitive;
use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;

/// Errors that may be returned by the TokenLending program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum EscrowError {
    /// Transaction signer does not have permission to execute instruction
    #[error("Unauthorized signer")]
    UnauthorizedSigner,

    /// Escrow already expired, all instructions disabled
    #[error("Escrow expired")]
    EscrowExpired,

    /// Individual or sum of stakes out of 0% to 100% bounds
    #[error("Stake out of bounds")]
    StakeOutOfBounds,

    /// This program is not the owner of the token account
    #[error("Token account authority")]
    TokenAccountAuthority,

    /// Token account has the wrong mint address
    #[error("Wrong token mint")]
    WrongTokenMint,

    /// Wrong escrow state
    #[error("Wrong state")]
    WrongState,

    /// Not enough balance on source account
    #[error("Not enough balance")]
    NotEnoughBalance,

    /// Reputation and recording oracle accounts must be initialized
    #[error("Oracle not initialized")]
    OracleNotInitialized,

    /// Too many payouts
    #[error("Too many payouts")]
    TooManyPayouts,

    /// Factory isn't initialized
    #[error("Factory isn't initialized")]
    FactoryNotInitialized,
}

const BASE_ERROR_CODE: u32 = 0x100;

impl From<EscrowError> for ProgramError {
    fn from(e: EscrowError) -> Self {
        ProgramError::Custom(BASE_ERROR_CODE + e as u32)
    }
}

impl<T> DecodeError<T> for EscrowError {
    fn type_of() -> &'static str {
        "Escrow Error"
    }
}
