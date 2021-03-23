use crate::*;

use crate::data::*;
use crate::responses::*;
use hmt_escrow::instruction::factory_initialize;
use hmt_escrow::state::Factory;
use rocket::State;
use rocket_contrib::json::Json;
use solana_program::instruction::Instruction;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};

///  Returns addresses of all jobs deployed in the factory
#[get("/factory?<address>")]
pub fn get_factory(address: String, config: State<Config>) -> Result<OkResponse, ErrorResponse> {
    let human_protocol_program = Pubkey::from_str(&config.human_protocol_program).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let memcp = Memcmp {
        offset: config.data_offset_to_begin_match,
        bytes: MemcmpEncodedBytes::Binary(address),
        encoding: None,
    };
    let filters = RpcFilterType::Memcmp(memcp);
    let configs = RpcProgramAccountsConfig {
        filters: Some(vec![filters]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            commitment: Some(CommitmentConfig::default()),
            ..RpcAccountInfoConfig::default()
        },
    };
    let accounts_with_config = config
        .rpc_client
        .get_program_accounts_with_config(&human_protocol_program, configs)
        .map_err(|e| {
            ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
                parameter_name: "address".to_string(),
                error: e.to_string(),
            }))
        })?;

    Ok(OkResponse::FactoryJobsResponse(Json(FactoryJobs {
        jobs: accounts_with_config
            .iter()
            .map(|account_data| account_data.0.to_string())
            .collect::<Vec<_>>(),
    })))
}

/// Creates a new factory and returns the address
#[post("/factory", format = "json", data = "<init_args>")]
pub fn new_factory(
    init_args: Json<InitFactoryArgs>,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let payer = Keypair::from_base58_string(&init_args.gasPayerPrivate);

    let factory_acc = Keypair::new();

    let factory_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Factory::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let instructions: Vec<Instruction> = vec![
        // Create Factory account
        system_instruction::create_account(
            &payer.pubkey(),
            &factory_acc.pubkey(),
            factory_account_balance,
            Factory::LEN as u64,
            &hmt_escrow::id(),
        ),
        // Initialize Factory account
        factory_initialize(
            &hmt_escrow::id(),
            &factory_acc.pubkey(),
            config.factory_version,
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?,
    ];

    let signers = vec![&payer, &factory_acc];

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));

    let (recent_blockhash, fee_calculator) =
        config.rpc_client.get_recent_blockhash().map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    helpers::check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        factory_account_balance + fee_calculator.calculate_fee(&transaction.message()),
    )
    .map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "gasPayerPrivate".to_string(),
            error: e.to_string(),
        }))
    })?;
    transaction.sign(&signers, recent_blockhash);

    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    Ok(OkResponse::DataResponse(Json(Response {
        data: factory_acc.pubkey().to_string(),
    })))
}
