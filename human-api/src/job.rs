use crate::data::*;
use crate::helpers::*;
use crate::responses::*;
use crate::Config;
use hmt_escrow::{
    instruction::cancel as cancel_escrow,
    instruction::complete as complete_escrow,
    instruction::initialize as initialize_escrow,
    instruction::payout,
    instruction::setup as setup_escrow,
    instruction::store_amounts,
    instruction::store_results,
    processor::Processor as EscrowProcessor,
    state::{DataHash, DataUrl, Escrow},
};
use rocket::State;
use rocket_contrib::json::Json;
use sha1::{Digest, Sha1};
use solana_program::{instruction::Instruction, program_pack::Pack, pubkey::Pubkey};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::{state::Account as TokenAccount, state::Mint as TokenMint};
use std::collections::HashMap;
use std::str::FromStr;

/// Creates a new job and returns the address
#[post("/job", format = "json", data = "<job_init_args>")]
pub fn new_job(
    job_init_args: Json<InitJobArgs>,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let payer = Keypair::from_base58_string(&job_init_args.gasPayerPrivate);
    let factory_pub_key = Pubkey::from_str(&job_init_args.factoryAddress).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "factoryAddress".to_string(),
            error: e.to_string(),
        }))
    })?;

    let mut instructions = vec![];
    let mut signers = vec![];
    let mut total_rent_free_balances = 0;

    let escrow_mint_account = Keypair::new();
    instructions.extend(create_mint(
        &config,
        &payer,
        &escrow_mint_account,
        &payer.pubkey(),
        config.token_decimals,
    )?);
    signers.extend(vec![&payer, &escrow_mint_account]);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenMint::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let escrow_account = Keypair::new();
    instructions.extend(create_escrow_account(&config, &payer, &escrow_account)?);
    signers.push(&escrow_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Escrow::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    // Calculate withdraw authority used for minting pool tokens
    let (authority, _) =
        EscrowProcessor::find_authority_bump_seed(&hmt_escrow::id(), &escrow_account.pubkey());

    let escrow_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &escrow_token_account,
        &escrow_mint_account.pubkey(),
        &authority,
    )?);
    signers.push(&escrow_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let canceler_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &canceler_token_account,
        &canceler_token_account.pubkey(),
        &payer.pubkey(),
    )?);
    signers.push(&canceler_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    // Initialize Escrow
    instructions.push(
        initialize_escrow(
            &hmt_escrow::id(),
            &escrow_account.pubkey(),
            &factory_pub_key,
            &escrow_mint_account.pubkey(),
            &escrow_token_account.pubkey(),
            &payer.pubkey(),
            &payer.pubkey(),
            &escrow_token_account.pubkey(),
            config.escrow_duration,
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?,
    );

    let manifest_url = DataUrl::from_str(&job_init_args.manifestUrl).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "manifestUrl".to_string(),
            error: e.to_string(),
        }))
    })?;

    let manifest_data: Manifest = reqwest::blocking::get(&job_init_args.manifestUrl)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?
        .json()
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let str_manifest_data = serde_json::to_string(&manifest_data).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let mut hasher = Sha1::new();
    hasher.update(str_manifest_data);
    let manifest_hash = DataHash::new_from_slice(&hasher.finalize()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let reputation_oracle_account_pub_key =
        Pubkey::from_str(&job_init_args.repOraclePub).map_err(|e| {
            ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
                parameter_name: "repOraclePub".to_string(),
                error: e.to_string(),
            }))
        })?;
    let reputation_oracle_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &reputation_oracle_token_account,
        &escrow_mint_account.pubkey(),
        &reputation_oracle_account_pub_key,
    )?);
    signers.push(&reputation_oracle_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let recording_oracle_account_pub_key = Pubkey::from_str(&manifest_data.recording_oracle_addr)
        .map_err(|_| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "recording_oracle_addr".to_string(),
            error: "Got wrong address from manifest url".to_string(),
        }))
    })?;
    let recording_oracle_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &recording_oracle_token_account,
        &escrow_mint_account.pubkey(),
        &recording_oracle_account_pub_key,
    )?);
    signers.push(&recording_oracle_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    // Setup Escrow
    instructions.push(
        setup_escrow(
            &hmt_escrow::id(),
            &escrow_account.pubkey(),
            &payer.pubkey(),
            &reputation_oracle_account_pub_key,
            &reputation_oracle_token_account.pubkey(),
            (manifest_data.oracle_stake * 100.0) as u8,
            &recording_oracle_account_pub_key,
            &reputation_oracle_token_account.pubkey(),
            (manifest_data.oracle_stake * 100.0) as u8,
            &manifest_url,
            &manifest_hash,
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?,
    );

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));

    let (recent_blockhash, fee_calculator) =
        config.rpc_client.get_recent_blockhash().map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
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
        data: escrow_account.pubkey().to_string(),
    })))
}

/// Retrieve the address of the launcher of a given job address
#[get("/launcher?<address>")]
pub fn get_job_launcher(
    address: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    Ok(OkResponse::DataResponse(Json(Response {
        data: escrow_info.launcher.to_string(),
    })))
}

/// Retrieve the status of a given job address
#[get("/status?<address>")]
pub fn get_job_status(address: String, config: State<Config>) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    Ok(OkResponse::StatusResponse(Json(StatusResponse {
        status: format!("{:?}", escrow_info.state),
    })))
}

/// Retrieve the Manifest URL of a given job address
#[get("/manifestUrl?<address>")]
pub fn get_job_manifest_url(
    address: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    Ok(OkResponse::DataResponse(Json(Response {
        data: escrow_info.manifest_url.to_string(),
    })))
}

/// Retrieve the Manifest Hash of a given job address
#[get("/manifestHash?<address>")]
pub fn get_job_manifest_hash(
    address: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    Ok(OkResponse::DataResponse(Json(Response {
        data: escrow_info.manifest_hash.to_string(),
    })))
}

/// Balance in HMT of a given job address
#[get("/balance?<address>")]
pub fn get_job_balance(
    address: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_account)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_token_account_info = TokenAccount::unpack_from_slice(account_data.as_slice())
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    Ok(OkResponse::BalanceResponse(Json(BalanceResponse {
        data: escrow_token_account_info.amount,
    })))
}

/// Abort a given job
#[allow(non_snake_case)]
#[get("/abort?<address>&<gasPayerPrivate>")]
pub fn abort_job(
    address: String,
    gasPayerPrivate: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;
    let payer = Keypair::from_base58_string(&gasPayerPrivate);

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_account)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_token_account_info = TokenAccount::unpack_from_slice(account_data.as_slice())
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    if escrow_token_account_info.amount != 0 {
        let authority = EscrowProcessor::authority_id(
            &hmt_escrow::id(),
            &escrow_pub_key,
            escrow_info.bump_seed,
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
        let mut transaction = Transaction::new_with_payer(
            &[cancel_escrow(
                &hmt_escrow::id(),
                &escrow_pub_key,
                &payer.pubkey(),
                &escrow_info.token_account,
                &authority,
                &escrow_info.canceler_token_account,
                &spl_token::id(),
            )
            .map_err(|e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?],
            Some(&payer.pubkey()),
        );
        let (recent_blockhash, fee_calculator) =
            config.rpc_client.get_recent_blockhash().map_err(|e| {
                ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?;
        check_fee_payer_balance(
            &config,
            &payer.pubkey(),
            fee_calculator.calculate_fee(&transaction.message()),
        )
        .map_err(|e| {
            ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
                parameter_name: "gasPayerPrivate".to_string(),
                error: e.to_string(),
            }))
        })?;
        transaction.sign(&vec![&payer], recent_blockhash);
        config
            .rpc_client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| {
                ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?;
    }

    Ok(OkResponse::BoolResponse(Json(BoolResponse {
        success: true,
    })))
}

/// Cancel a given job
#[allow(non_snake_case)]
#[get("/cancel?<address>&<gasPayerPrivate>")]
pub fn cancel_job(
    address: String,
    gasPayerPrivate: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;
    let payer = Keypair::from_base58_string(&gasPayerPrivate);

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let authority =
        EscrowProcessor::authority_id(&hmt_escrow::id(), &escrow_pub_key, escrow_info.bump_seed)
            .map_err(|e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?;
    let mut transaction = Transaction::new_with_payer(
        &[cancel_escrow(
            &hmt_escrow::id(),
            &escrow_pub_key,
            &payer.pubkey(),
            &escrow_info.token_account,
            &authority,
            &escrow_info.canceler_token_account,
            &spl_token::id(),
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?],
        Some(&payer.pubkey()),
    );
    let (recent_blockhash, fee_calculator) =
        config.rpc_client.get_recent_blockhash().map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        fee_calculator.calculate_fee(&transaction.message()),
    )
    .map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "gasPayerPrivate".to_string(),
            error: e.to_string(),
        }))
    })?;
    transaction.sign(&vec![&payer], recent_blockhash);
    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    Ok(OkResponse::BoolResponse(Json(BoolResponse {
        success: true,
    })))
}

/// Complete a given job
#[allow(non_snake_case)]
#[get("/complete?<address>&<gasPayerPrivate>")]
pub fn complete_job(
    address: String,
    gasPayerPrivate: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;
    let payer = Keypair::from_base58_string(&gasPayerPrivate);

    let mut transaction = Transaction::new_with_payer(
        &[
            complete_escrow(&hmt_escrow::id(), &escrow_pub_key, &payer.pubkey()).map_err(|e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?,
        ],
        Some(&payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) =
        config.rpc_client.get_recent_blockhash().map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        fee_calculator.calculate_fee(&transaction.message()),
    )
    .map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "gasPayerPrivate".to_string(),
            error: e.to_string(),
        }))
    })?;
    transaction.sign(&vec![&payer], recent_blockhash);
    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    Ok(OkResponse::BoolResponse(Json(BoolResponse {
        success: true,
    })))
}

/// Store job results
#[post(
    "/storeIntermediateResults",
    format = "json",
    data = "<store_results_args>"
)]
pub fn store_job_intermediate_results(
    store_results_args: Json<StoreResultsArgs>,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let payer = Keypair::from_base58_string(&store_results_args.gasPayerPrivate);
    let escrow_pub_key = Pubkey::from_str(&store_results_args.address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let results_data: ResultsData = reqwest::blocking::get(&store_results_args.resultsUrl)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?
        .json()
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let str_results_data = serde_json::to_string(&results_data).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let mut hasher = Sha1::new();
    hasher.update(str_results_data);
    let results_hash = DataHash::new_from_slice(&hasher.finalize()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let results_url = DataUrl::from_str(&store_results_args.resultsUrl).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    // Read escrow state to sure that it's initialized
    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    let mut transaction = Transaction::new_with_payer(
        &[
            // Store results instruction
            store_results(
                &hmt_escrow::id(),
                &escrow_pub_key,
                &payer.pubkey(),
                &results_url,
                &results_hash,
            )
            .map_err(|e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?,
        ],
        Some(&payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) =
        config.rpc_client.get_recent_blockhash().map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        fee_calculator.calculate_fee(&transaction.message()),
    )
    .map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "gasPayerPrivate".to_string(),
            error: e.to_string(),
        }))
    })?;
    transaction.sign(&vec![&payer], recent_blockhash);

    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    Ok(OkResponse::BoolResponse(Json(BoolResponse {
        success: true,
    })))
}

/// Performs a payout to multiple Solana addresses
#[post("/bulkPayout", format = "json", data = "<bulk_payout_args>")]
pub fn bulk_payout(
    bulk_payout_args: Json<BulkPayoutArgs>,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let payer = Keypair::from_base58_string(&bulk_payout_args.gasPayerPrivate);
    let escrow_pub_key = Pubkey::from_str(&bulk_payout_args.address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;
    let escrow_account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info: Escrow =
        Escrow::unpack_from_slice(escrow_account_data.as_slice()).map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let payouts_data: HashMap<String, String> =
        reqwest::blocking::get(&bulk_payout_args.payoutsUrl)
            .map_err(|e| {
                ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?
            .json()
            .map_err(|e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?;

    let recipients: Vec<PayoutRecord> = payouts_data
        .iter()
        .filter_map(|(pub_k, amount)| {
            let recipient: Option<Pubkey> = Pubkey::from_str(&pub_k).ok();
            let amount: Option<f64> = amount.parse::<f64>().ok();
            match (recipient, amount) {
                (Some(recipient), Some(amount)) => Some(PayoutRecord { recipient, amount }),
                _ => None,
            }
        })
        .collect();

    if recipients.is_empty() {
        return Err(ErrorResponse::InvalidParameterResponse(Json(
            InvalidParameter {
                parameter_name: "payoutsUrl".to_string(),
                error: "Cannot find anyone to sent tokens to".to_string(),
            },
        )));
    }

    let total_amount: f64 = recipients.iter().map(|x| x.amount).sum();

    // Check token mint to convert amount to u64
    let token_mint_account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_mint)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let mint_info: TokenMint = TokenMint::unpack_from_slice(token_mint_account_data.as_slice())
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    let total_amount = spl_token::ui_amount_to_amount(total_amount, mint_info.decimals);

    let mut instructions = vec![];
    let mut signers = vec![];

    instructions.push(
        store_amounts(
            &hmt_escrow::id(),
            &escrow_pub_key,
            &payer.pubkey(),
            total_amount,
            recipients.len() as u64,
        )
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?,
    );
    signers.push(&payer);

    let reputation_oracle_token_account = escrow_info.reputation_oracle_token_account.ok_or(
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: "Escrow doesn't have reputation oracle token account".to_string(),
        })),
    )?;
    let recording_oracle_token_account = escrow_info.recording_oracle_token_account.ok_or(
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: "Escrow doesn't have recording oracle token account".to_string(),
        })),
    )?;

    // Check escrow token account balance
    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_account)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let token_account_info: TokenAccount = TokenAccount::unpack_from_slice(account_data.as_slice())
        .map_err(|e| {
            ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;

    if total_amount > token_account_info.amount {
        return Err(ErrorResponse::InvalidParameterResponse(Json(
            InvalidParameter {
                parameter_name: "address".to_string(),
                error: "Escrow token account doesn't have enough tokens to do payout".to_string(),
            },
        )));
    }
    let authority =
        EscrowProcessor::authority_id(&hmt_escrow::id(), &escrow_pub_key, escrow_info.bump_seed)
            .map_err(|e| {
                ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
                    error: e.to_string(),
                }))
            })?;

    let payout_instructions: Vec<Instruction> = recipients
        .iter()
        .filter_map(|record| {
            payout(
                &hmt_escrow::id(),
                &escrow_pub_key,
                &payer.pubkey(),
                &escrow_info.token_account,
                &authority,
                &record.recipient,
                &reputation_oracle_token_account,
                &recording_oracle_token_account,
                &spl_token::id(),
                spl_token::ui_amount_to_amount(record.amount, mint_info.decimals),
            )
            .ok()
        })
        .collect();

    if payout_instructions.is_empty() {
        return Err(ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: "Error occurs while construct payout instruction".to_string(),
        })));
    }

    instructions.extend(payout_instructions);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    let (recent_blockhash, fee_calculator) =
        config.rpc_client.get_recent_blockhash().map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        fee_calculator.calculate_fee(&transaction.message()),
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

    Ok(OkResponse::BoolResponse(Json(BoolResponse {
        success: true,
    })))
}

/// Add trusted handlers that can freely transact with the contract
#[post(
    "/addTrustedHandlers",
    format = "json",
    data = "<_trusted_handlers_args>"
)]
pub fn add_trusted_handlers(
    _trusted_handlers_args: Json<TrustedHandlersArgs>,
) -> Result<OkResponse, ErrorResponse> {
    unimplemented!();
}

/// Retrieve the intermediate results stored by the Recording Oracle
#[get("/intermediateResults?<_address>")]
pub fn get_intermediate_results(_address: String) -> Result<OkResponse, ErrorResponse> {
    unimplemented!();
}

/// Retrieve the final results
#[get("/finalResults?<address>")]
pub fn get_final_results(
    address: String,
    config: State<Config>,
) -> Result<OkResponse, ErrorResponse> {
    let escrow_pub_key = Pubkey::from_str(&address).map_err(|e| {
        ErrorResponse::InvalidParameterResponse(Json(InvalidParameter {
            parameter_name: "address".to_string(),
            error: e.to_string(),
        }))
    })?;

    let account_data = config
        .rpc_client
        .get_account_data(&escrow_pub_key)
        .map_err(|e| {
            ErrorResponse::BadGatewayErrorResponse(Json(ErrorMessage {
                error: e.to_string(),
            }))
        })?;
    let escrow_info = Escrow::unpack_from_slice(account_data.as_slice()).map_err(|e| {
        ErrorResponse::ServerErrorResponse(Json(ErrorMessage {
            error: e.to_string(),
        }))
    })?;

    Ok(OkResponse::DataResponse(Json(Response {
        data: escrow_info.final_results_url.to_string(),
    })))
}
