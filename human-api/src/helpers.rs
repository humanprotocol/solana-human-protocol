use crate::responses::*;
use crate::*;
use hmt_escrow::state::Escrow;
use rocket_contrib::json::Json;
use solana_program::{instruction::Instruction, program_pack::Pack, pubkey::Pubkey};
use solana_sdk::{
    native_token::*,
    signature::{Keypair, Signer},
    system_instruction,
};
use spl_token::{
    instruction::initialize_account, instruction::initialize_mint, state::Account as TokenAccount,
    state::Mint as TokenMint,
};

pub fn check_fee_payer_balance(
    config: &Config,
    address: &Pubkey,
    required_balance: u64,
) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(address)?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            address,
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

pub fn create_mint(
    config: &Config,
    payer: &Keypair,
    mint_account: &Keypair,
    owner: &Pubkey,
    decimals: u8,
) -> Result<Vec<Instruction>, ErrorResponse> {
    let mint_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenMint::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let instructions = vec![
        // Account for token mint
        system_instruction::create_account(
            &payer.pubkey(),
            &mint_account.pubkey(),
            mint_account_balance,
            TokenMint::LEN as u64,
            &spl_token::id(),
        ),
        // Create mint account
        initialize_mint(
            &spl_token::id(),
            &mint_account.pubkey(),
            owner,
            None,
            decimals,
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?,
    ];

    Ok(instructions)
}

pub fn create_token_account(
    config: &Config,
    payer: &Keypair,
    token_account: &Keypair,
    token_mint: &Pubkey,
    owner: &Pubkey,
) -> Result<Vec<Instruction>, ErrorResponse> {
    let token_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let instructions = vec![
        // Create system account first
        system_instruction::create_account(
            &payer.pubkey(),
            &token_account.pubkey(),
            token_account_balance,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        ),
        // Initialize token account
        initialize_account(&spl_token::id(), &token_account.pubkey(), token_mint, owner).map_err(
            |e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            },
        )?,
    ];

    Ok(instructions)
}

pub fn create_escrow_account(
    config: &Config,
    payer: &Keypair,
    escrow_account: &Keypair,
) -> Result<Vec<Instruction>, ErrorResponse> {
    let escrow_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Escrow::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let instruction = vec![
        // Create system account for Escrow
        system_instruction::create_account(
            &payer.pubkey(),
            &escrow_account.pubkey(),
            escrow_account_balance,
            Escrow::LEN as u64,
            &hmt_escrow::id(),
        ),
    ];

    Ok(instruction)
}
