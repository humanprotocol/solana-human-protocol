//! Program state processor

use crate::error::EscrowError;
use crate::instruction::EscrowInstruction;
use crate::state::*;
use num_traits::FromPrimitive;
use solana_program::program::invoke_signed;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    msg,
    program_error::{PrintProgramError, ProgramError},
    program_option::COption,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use spl_token::state::Account as TokenAccount;

/// Program state handler.
pub struct Processor {}

impl Processor {
    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        escrow_program_id: &Pubkey,
        escrow_account_key: &Pubkey,
        bump_seed: u8,
    ) -> Result<Pubkey, ProgramError> {
        Pubkey::create_program_address(
            &[&escrow_account_key.to_bytes()[..32], &[bump_seed]],
            escrow_program_id,
        )
        .or(Err(ProgramError::IncorrectProgramId))
    }

    /// Generates seed bump for escrow authority
    pub fn find_authority_bump_seed(
        escrow_program_id: &Pubkey,
        escrow_account_key: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[&escrow_account_key.to_bytes()[..32]], escrow_program_id)
    }

    /// Verifies if transaction is signed by the trusted handler
    fn check_trusted_handler(escrow: &Escrow, trusted_handler_info: &AccountInfo) -> ProgramResult {
        // Check if instruction is signed by the trusted handler
        if !trusted_handler_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Check is signer is either launcher or canceler authority
        if *trusted_handler_info.key == escrow.launcher {
            return Ok(());
        }
        if *trusted_handler_info.key == escrow.canceler {
            return Ok(());
        }

        // Check for reputation and recording oracles
        if let COption::Some(pubkey) = escrow.reputation_oracle {
            if *trusted_handler_info.key == pubkey {
                return Ok(());
            }
        }
        if let COption::Some(pubkey) = escrow.recording_oracle {
            if *trusted_handler_info.key == pubkey {
                return Ok(());
            }
        }

        // Trusted handler not recognized
        Err(EscrowError::UnauthorizedSigner.into())
    }

    fn get_escrow_with_state_check(
        escrow_info: &AccountInfo,
        clock: &Clock,
        trusted_handler_info: &AccountInfo,
        allowed_states: Vec<EscrowState>,
    ) -> Result<Escrow, ProgramError> {
        let escrow = Escrow::unpack_unchecked(&escrow_info.data.borrow())?;

        // Check if escrow account exists and is initialized
        if !escrow.is_initialized() {
            return Err(ProgramError::UninitializedAccount);
        }

        // Check escrow account expiration
        if escrow.expires < clock.unix_timestamp {
            return Err(EscrowError::EscrowExpired.into());
        }

        // Check escrow state
        if !allowed_states.contains(&escrow.state) {
            return Err(EscrowError::WrongState.into());
        }

        Self::check_trusted_handler(&escrow, trusted_handler_info)?;

        Ok(escrow)
    }

    /// Issue a spl_token `Transfer` instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn token_transfer<'a>(
        escrow_account_key: &Pubkey,
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        bump_seed: u8,
        amount: u64,
    ) -> ProgramResult {
        let authority_signature_seeds = [&escrow_account_key.to_bytes()[..32], &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }

    /// Processes `FactoryInitialize` instruction.
    pub fn process_factory_initialize(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        version: u8,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let factory_info = next_account_info(account_info_iter)?;

        let factory = Factory::unpack_unchecked(&factory_info.data.borrow())?;

        // Only new unitialized accounts are supported
        if factory.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        let factory = Factory { version };

        Factory::pack(factory, &mut factory_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes `Initialize` instruction.
    pub fn process_initialize(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        duration: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let factory_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

        let token_mint_info = next_account_info(account_info_iter)?;
        let token_account_info = next_account_info(account_info_iter)?;
        let launcher_info = next_account_info(account_info_iter)?;
        let canceler_info = next_account_info(account_info_iter)?;
        let canceler_token_account_info = next_account_info(account_info_iter)?;

        let escrow = Box::new(Escrow::unpack_unchecked(&escrow_info.data.borrow())?);

        let factory = Factory::unpack(&factory_info.data.borrow())?;

        // Only new unitialized accounts are supported
        if escrow.is_initialized() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        // Escrow has to belong to initialized Factory
        if !factory.is_initialized() {
            return Err(EscrowError::FactoryNotInitialized.into());
        }

        // Check duration validity
        if duration == 0 {
            return Err(EscrowError::EscrowExpired.into());
        }

        // Calculate authority key and bump seed
        let (authority_key, bump_seed) =
            Self::find_authority_bump_seed(program_id, escrow_info.key);

        // Token account should be owned by the contract authority
        let token_account = Box::new(TokenAccount::unpack_unchecked(
            &token_account_info.data.borrow(),
        )?);
        if token_account.owner != authority_key {
            return Err(EscrowError::TokenAccountAuthority.into());
        }

        // Check token account mints
        if token_account.mint != *token_mint_info.key {
            return Err(EscrowError::WrongTokenMint.into());
        }
        let canceler_token_account = Box::new(TokenAccount::unpack_unchecked(
            &canceler_token_account_info.data.borrow(),
        )?);
        if canceler_token_account.mint != *token_mint_info.key {
            return Err(EscrowError::WrongTokenMint.into());
        }

        let escrow = Box::new(Escrow {
            state: EscrowState::Launched,
            expires: clock.unix_timestamp + duration as i64,
            bump_seed,
            token_mint: *token_mint_info.key,
            token_account: *token_account_info.key,
            launcher: *launcher_info.key,
            canceler: *canceler_info.key,
            canceler_token_account: *canceler_token_account_info.key,
            ..Default::default()
        });

        Escrow::pack(*escrow, &mut escrow_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes `Setup` instruction.
    pub fn process_setup(
        accounts: &[AccountInfo],
        reputation_oracle_stake: u8,
        recording_oracle_stake: u8,
        manifest_url: &DataUrl,
        manifest_hash: &DataHash,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let trusted_handler_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

        let reputation_oracle_info = next_account_info(account_info_iter)?;
        let reputation_oracle_token_account_info = next_account_info(account_info_iter)?;
        let recording_oracle_info = next_account_info(account_info_iter)?;
        let recording_oracle_token_account_info = next_account_info(account_info_iter)?;

        let mut escrow = Self::get_escrow_with_state_check(
            escrow_info,
            clock,
            trusted_handler_info,
            vec![EscrowState::Launched],
        )?;

        // Check stake value validity
        let total_stake: u8 = reputation_oracle_stake
            .checked_add(recording_oracle_stake)
            .ok_or(ProgramError::InvalidInstructionData)?;
        if total_stake == 0 || total_stake > 100 {
            return Err(EscrowError::StakeOutOfBounds.into());
        }

        // Check token account mints
        let reputation_oracle_token_account =
            TokenAccount::unpack_unchecked(&reputation_oracle_token_account_info.data.borrow())?;
        if reputation_oracle_token_account.mint != escrow.token_mint {
            return Err(EscrowError::WrongTokenMint.into());
        }
        let recording_oracle_token_account =
            TokenAccount::unpack_unchecked(&recording_oracle_token_account_info.data.borrow())?;
        if recording_oracle_token_account.mint != escrow.token_mint {
            return Err(EscrowError::WrongTokenMint.into());
        }

        // Update escrow fields with the new values
        escrow.reputation_oracle = COption::Some(*reputation_oracle_info.key);
        escrow.reputation_oracle_token_account =
            COption::Some(*reputation_oracle_token_account_info.key);
        escrow.reputation_oracle_stake = reputation_oracle_stake;

        escrow.recording_oracle = COption::Some(*recording_oracle_info.key);
        escrow.recording_oracle_token_account =
            COption::Some(*recording_oracle_token_account_info.key);
        escrow.recording_oracle_stake = recording_oracle_stake;

        escrow.manifest_url = *manifest_url;
        escrow.manifest_hash = *manifest_hash;

        escrow.state = EscrowState::Pending;

        Escrow::pack(escrow, &mut escrow_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes `StoreResults` instruction.
    pub fn process_store_results(
        accounts: &[AccountInfo],
        final_results_url: &DataUrl,
        final_results_hash: &DataHash,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let trusted_handler_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

        let mut escrow = Self::get_escrow_with_state_check(
            escrow_info,
            clock,
            trusted_handler_info,
            vec![EscrowState::Pending, EscrowState::Partial],
        )?;

        // Save final results url and hash
        escrow.final_results_url = *final_results_url;
        escrow.final_results_hash = *final_results_hash;

        Escrow::pack(escrow, &mut escrow_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes `StoreFinalAmounts` instruction.
    pub fn process_store_amounts(
        accounts: &[AccountInfo],
        total_amount: u64,
        total_recipients: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let trusted_handler_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

        let mut escrow = Self::get_escrow_with_state_check(
            escrow_info,
            clock,
            trusted_handler_info,
            vec![EscrowState::Pending, EscrowState::Partial],
        )?;

        // Save final results url and hash
        escrow.total_amount = total_amount;
        escrow.total_recipients = total_recipients;

        Escrow::pack(escrow, &mut escrow_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes `Payout` instruction.
    pub fn process_payout(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        amount: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let trusted_handler_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
        let token_account_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let recipient_token_account_info = next_account_info(account_info_iter)?;
        let reputation_oracle_token_account_info = next_account_info(account_info_iter)?;
        let recording_oracle_token_account_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let mut escrow = Self::get_escrow_with_state_check(
            escrow_info,
            clock,
            trusted_handler_info,
            vec![EscrowState::Pending, EscrowState::Partial],
        )?;

        // Check all accounts validity
        if *token_account_info.key != escrow.token_account
            || *reputation_oracle_token_account_info.key
                != escrow
                    .reputation_oracle_token_account
                    .ok_or(EscrowError::OracleNotInitialized)?
            || *recording_oracle_token_account_info.key
                != escrow
                    .recording_oracle_token_account
                    .ok_or(EscrowError::OracleNotInitialized)?
            || *authority_info.key
                != Self::authority_id(program_id, escrow_info.key, escrow.bump_seed)?
        {
            return Err(ProgramError::InvalidInstructionData);
        }

        // Check account balance
        let token_account = TokenAccount::unpack_unchecked(&token_account_info.data.borrow())?;
        if token_account.amount < amount {
            return Err(EscrowError::NotEnoughBalance.into());
        }

        // Check if not too many payouts
        if (escrow.sent_amount + amount > escrow.total_amount)
            || (escrow.sent_recipients + 1 > escrow.total_recipients)
        {
            return Err(EscrowError::TooManyPayouts.into());
        }

        // Calculate fees
        let reputation_oracle_fee_amount = amount
            .checked_mul(escrow.reputation_oracle_stake as u64)
            .unwrap_or(0)
            .checked_div(100)
            .unwrap_or(0);
        let recording_oracle_fee_amount = amount
            .checked_mul(escrow.recording_oracle_stake as u64)
            .unwrap_or(0)
            .checked_div(100)
            .unwrap_or(0);
        let recipient_amount = amount
            .saturating_sub(reputation_oracle_fee_amount)
            .saturating_sub(recording_oracle_fee_amount);

        // Send tokens
        if recipient_amount != 0 {
            Self::token_transfer(
                escrow_info.key,
                token_program_info.clone(),
                token_account_info.clone(),
                recipient_token_account_info.clone(),
                authority_info.clone(),
                escrow.bump_seed,
                recipient_amount,
            )?;
        }
        if reputation_oracle_fee_amount != 0 {
            Self::token_transfer(
                escrow_info.key,
                token_program_info.clone(),
                token_account_info.clone(),
                reputation_oracle_token_account_info.clone(),
                authority_info.clone(),
                escrow.bump_seed,
                reputation_oracle_fee_amount,
            )?;
        }
        if recording_oracle_fee_amount != 0 {
            Self::token_transfer(
                escrow_info.key,
                token_program_info.clone(),
                token_account_info.clone(),
                recording_oracle_token_account_info.clone(),
                authority_info.clone(),
                escrow.bump_seed,
                recording_oracle_fee_amount,
            )?;
        }

        escrow.sent_amount += amount;
        escrow.sent_recipients += 1;

        if escrow.sent_recipients == escrow.total_recipients
            && escrow.sent_amount == escrow.total_amount
        {
            escrow.state = EscrowState::Paid;
        } else {
            escrow.state = EscrowState::Partial;
        }

        Escrow::pack(escrow, &mut escrow_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes `Cancel` instruction.
    pub fn process_cancel(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let trusted_handler_info = next_account_info(account_info_iter)?;
        let token_account_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let canceler_token_account_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let mut escrow = Escrow::unpack_unchecked(&escrow_info.data.borrow())?;

        // Check if escrow account exists and is initialized
        if !escrow.is_initialized() {
            return Err(ProgramError::UninitializedAccount);
        }

        // Check escrow state
        if escrow.state == EscrowState::Complete || escrow.state == EscrowState::Paid {
            return Err(EscrowError::WrongState.into());
        }

        Self::check_trusted_handler(&escrow, trusted_handler_info)?;

        // Check all accounts validity
        if *token_account_info.key != escrow.token_account
            || *canceler_token_account_info.key != escrow.canceler_token_account
            || *authority_info.key
                != Self::authority_id(program_id, escrow_info.key, escrow.bump_seed)?
        {
            return Err(ProgramError::InvalidInstructionData);
        }

        // Check account balance
        let token_account = TokenAccount::unpack_unchecked(&token_account_info.data.borrow())?;
        if token_account.amount == 0 {
            return Err(EscrowError::NotEnoughBalance.into());
        }

        // Call token contract to do transfer
        Self::token_transfer(
            escrow_info.key,
            token_program_info.clone(),
            token_account_info.clone(),
            canceler_token_account_info.clone(),
            authority_info.clone(),
            escrow.bump_seed,
            token_account.amount,
        )?;

        escrow.state = EscrowState::Cancelled;

        Escrow::pack(escrow, &mut escrow_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes `Complete` instruction.
    pub fn process_complete(accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_info = next_account_info(account_info_iter)?;
        let trusted_handler_info = next_account_info(account_info_iter)?;
        let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;

        let mut escrow = Self::get_escrow_with_state_check(
            escrow_info,
            clock,
            trusted_handler_info,
            vec![EscrowState::Paid],
        )?;

        escrow.state = EscrowState::Complete;

        Escrow::pack(escrow, &mut escrow_info.data.borrow_mut())?;

        Ok(())
    }

    /// Processes all Escrow instructions
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = EscrowInstruction::unpack(input)?;

        match instruction {
            EscrowInstruction::FactoryInitialize { version } => {
                msg!("Instruction: Initialize Factory");
                Self::process_factory_initialize(program_id, accounts, version)
            }
            EscrowInstruction::Initialize { duration } => {
                msg!("Instruction: Initialize");
                Self::process_initialize(program_id, accounts, duration)
            }
            EscrowInstruction::Setup {
                reputation_oracle_stake,
                recording_oracle_stake,
                manifest_url,
                manifest_hash,
            } => {
                msg!("Instruction: Setup");
                Self::process_setup(
                    accounts,
                    reputation_oracle_stake,
                    recording_oracle_stake,
                    &manifest_url,
                    &manifest_hash,
                )
            }
            EscrowInstruction::StoreResults {
                final_results_url,
                final_results_hash,
            } => {
                msg!("Instruction: Store Results");
                Self::process_store_results(
                    accounts,
                    &final_results_url,
                    &final_results_hash,
                )
            }
            EscrowInstruction::StoreFinalAmounts {
                total_amount,
                total_recipients,
            } => {
                msg!("Instruction: Store Amounts");
                Self::process_store_amounts(
                    accounts,
                    total_amount,
                    total_recipients,
                )
            }
            EscrowInstruction::Payout { amount } => {
                msg!("Instruction: Payout");
                Self::process_payout(program_id, accounts, amount)
            }
            EscrowInstruction::Cancel => {
                msg!("Instruction: Payout");
                Self::process_cancel(program_id, accounts)
            }
            EscrowInstruction::Complete => {
                msg!("Instruction: Payout");
                Self::process_complete(accounts)
            }
        }
    }
}

impl PrintProgramError for EscrowError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            EscrowError::UnauthorizedSigner => msg!("Error: unauthorized signer"),
            EscrowError::EscrowExpired => msg!("Error: escrow expired"),
            EscrowError::StakeOutOfBounds => msg!("Error: stake out of bounds"),
            EscrowError::TokenAccountAuthority => msg!("Error: token account authority"),
            EscrowError::WrongTokenMint => msg!("Error: wrong token mint"),
            EscrowError::WrongState => msg!("Error: wrong escrow state"),
            EscrowError::NotEnoughBalance => msg!("Error: not enough balance"),
            EscrowError::OracleNotInitialized => msg!("Error: oracle not initialized"),
            EscrowError::TooManyPayouts => msg!("Error: too many payouts"),
            EscrowError::FactoryNotInitialized => msg!("Factory isn't initialized"),
        }
    }
}
