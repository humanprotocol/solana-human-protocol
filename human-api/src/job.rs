use crate::data::*;
use crate::helpers::*;
use crate::Config;
use hmt_escrow::{
    instruction::initialize as initialize_escrow,
    instruction::payout,
    instruction::setup as setup_escrow,
    instruction::store_amounts,
    instruction::store_results,
    processor::Processor as EscrowProcessor,
    state::{DataHash, DataUrl, Escrow},
};
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};
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
pub fn new_job(job_init_args: Json<InitJobArgs>, config: State<Config>) -> Json<Response> {
    let payer = Keypair::from_base58_string(&job_init_args.gasPayerPrivate);
    let factory_pub_key = Pubkey::from_str(&job_init_args.factoryAddress).unwrap();

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
    ));
    signers.extend(vec![&payer, &escrow_mint_account]);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenMint::LEN)
        .unwrap();

    let escrow_account = Keypair::new();
    instructions.extend(create_escrow_account(&config, &payer, &escrow_account));
    signers.push(&escrow_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Escrow::LEN)
        .unwrap();

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
    ));
    signers.push(&escrow_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .unwrap();

    let canceler_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &canceler_token_account,
        &canceler_token_account.pubkey(),
        &payer.pubkey(),
    ));
    signers.push(&canceler_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .unwrap();

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
        .unwrap(),
    );

    let manifest_data: Manifest = reqwest::blocking::get(&job_init_args.manifestUrl)
        .unwrap()
        .json()
        .unwrap();
    let str_manifest_data = serde_json::to_string(&manifest_data).unwrap();

    let mut hasher = Sha1::new();
    hasher.update(str_manifest_data);
    let manifest_hash = DataHash::new_from_slice(&hasher.finalize()).unwrap();

    let reputation_oracle_account_pub_key = Pubkey::from_str(&job_init_args.repOraclePub).unwrap();
    let reputation_oracle_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &reputation_oracle_token_account,
        &escrow_mint_account.pubkey(),
        &reputation_oracle_account_pub_key,
    ));
    signers.push(&reputation_oracle_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .unwrap();

    let recording_oracle_account_pub_key =
        Pubkey::from_str(&manifest_data.recording_oracle_addr).unwrap();
    let recording_oracle_token_account = Keypair::new();
    instructions.extend(create_token_account(
        &config,
        &payer,
        &recording_oracle_token_account,
        &escrow_mint_account.pubkey(),
        &recording_oracle_account_pub_key,
    ));
    signers.push(&recording_oracle_token_account);
    total_rent_free_balances += config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .unwrap();

    let manifest_url = DataUrl::from_str(&job_init_args.manifestUrl).unwrap();

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
        .unwrap(),
    );

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash().unwrap();
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        total_rent_free_balances + fee_calculator.calculate_fee(&transaction.message()),
    )
    .unwrap();
    transaction.sign(&signers, recent_blockhash);

    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .unwrap();

    Json(Response {
        data: escrow_account.pubkey().to_string(),
    })
}

/// Receive the address of the launcher of a given job address
#[get("/launcher?<_address>")]
pub fn get_job_launcher(_address: String) -> JsonValue {
    unimplemented!();
}

/// Receive the status of a given job address
#[get("/status?<_address>")]
pub fn get_job_status(_address: String) -> JsonValue {
    unimplemented!();
}

/// Receive the Manifest URL of a given job address
#[get("/manifestUrl?<_address>")]
pub fn get_job_manifest_url(_address: String) -> JsonValue {
    unimplemented!();
}

/// Receive the Manifest Hash of a given job address
#[get("/manifestHash?<_address>")]
pub fn get_job_manifest_hash(_address: String) -> JsonValue {
    unimplemented!();
}

/// Balance in HMT of a given job address
#[get("/balance?<_address>")]
pub fn get_job_balance(_address: String) -> JsonValue {
    unimplemented!();
}

/// Abort a given job
#[get("/abort?<_address>")]
pub fn abort_job(_address: String) -> JsonValue {
    unimplemented!();
}

/// Cancel a given job
#[get("/cancel?<_address>")]
pub fn cancel_job(_address: String) -> JsonValue {
    unimplemented!();
}

/// Complete a given job
#[post("/complete?<_address>")]
pub fn complete_job(_address: String) -> JsonValue {
    unimplemented!();
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
) -> Json<BoolResponse> {
    let payer = Keypair::from_base58_string(&store_results_args.gasPayerPrivate);
    let escrow_pub_key = Pubkey::from_str(&store_results_args.address).unwrap();

    let results_data: ResultsData = reqwest::blocking::get(&store_results_args.resultsUrl)
        .unwrap()
        .json()
        .unwrap();
    let str_results_data = serde_json::to_string(&results_data).unwrap();

    let mut hasher = Sha1::new();
    hasher.update(str_results_data);
    let results_hash = DataHash::new_from_slice(&hasher.finalize()).unwrap();

    let results_url = DataUrl::from_str(&store_results_args.resultsUrl).unwrap();

    // Read escrow state to sure that it's initialized
    let account_data = config.rpc_client.get_account_data(&escrow_pub_key).unwrap();
    Escrow::unpack_from_slice(account_data.as_slice()).unwrap();

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
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash().unwrap();
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        fee_calculator.calculate_fee(&transaction.message()),
    )
    .unwrap();
    transaction.sign(&vec![&payer], recent_blockhash);

    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .unwrap();

    Json(BoolResponse { success: true })
}

/// Performs a payout to multiple Solana addresses
#[post("/bulkPayout", format = "json", data = "<bulk_payout_args>")]
pub fn bulk_payout(
    bulk_payout_args: Json<BulkPayoutArgs>,
    config: State<Config>,
) -> Json<BoolResponse> {
    let payer = Keypair::from_base58_string(&bulk_payout_args.gasPayerPrivate);
    let escrow_pub_key = Pubkey::from_str(&bulk_payout_args.address).unwrap();
    let escrow_account_data = config.rpc_client.get_account_data(&escrow_pub_key).unwrap();
    let escrow_info: Escrow = Escrow::unpack_from_slice(escrow_account_data.as_slice()).unwrap();

    let payouts_data: HashMap<String, String> =
        reqwest::blocking::get(&bulk_payout_args.payoutsUrl)
            .unwrap()
            .json()
            .unwrap();

    let recipients: Vec<PayoutRecord> = payouts_data
        .iter()
        .map(|(pub_k, amount)| PayoutRecord {
            recipient: Pubkey::from_str(&pub_k).unwrap(),
            amount: amount.parse::<f64>().unwrap(),
        })
        .collect();

    let total_amount: f64 = recipients.iter().map(|x| x.amount).sum();

    // Check token mint to convert amount to u64
    let token_mint_account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_mint)
        .unwrap();
    let mint_info: TokenMint =
        TokenMint::unpack_from_slice(token_mint_account_data.as_slice()).unwrap();

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
        .unwrap(),
    );
    signers.push(&payer);

    let reputation_oracle_token_account = escrow_info.reputation_oracle_token_account.unwrap();
    let recording_oracle_token_account = escrow_info.recording_oracle_token_account.unwrap();

    // Check escrow token account balance
    let account_data = config
        .rpc_client
        .get_account_data(&escrow_info.token_account)
        .unwrap();
    let token_account_info: TokenAccount =
        TokenAccount::unpack_from_slice(account_data.as_slice()).unwrap();

    if total_amount > token_account_info.amount {
        unimplemented!(); // TODO: return error message
    }
    let authority =
        EscrowProcessor::authority_id(&hmt_escrow::id(), &escrow_pub_key, escrow_info.bump_seed)
            .unwrap();

    let payout_instructions: Vec<Instruction> = recipients
        .iter()
        .map(|record| {
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
            .unwrap()
        })
        .collect();

    instructions.extend(payout_instructions);

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash().unwrap();
    check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        fee_calculator.calculate_fee(&transaction.message()),
    )
    .unwrap();
    transaction.sign(&signers, recent_blockhash);

    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .unwrap();

    Json(BoolResponse { success: true })
}

/// Add trusted handlers that can freely transact with the contract
#[post(
    "/addTrustedHandlers",
    format = "json",
    data = "<_trusted_handlers_args>"
)]
pub fn add_trusted_handlers(_trusted_handlers_args: Json<TrustedHandlersArgs>) -> JsonValue {
    unimplemented!();
}

/// Retrieve the intermediate results stored by the Recording Oracle
#[post("/intermediateResults?<_address>")]
pub fn get_intermediate_results(_address: String) -> JsonValue {
    unimplemented!();
}

/// Retrieve the final results
#[post("/finalResults?<_address>")]
pub fn get_final_results(_address: String) -> JsonValue {
    unimplemented!();
}
