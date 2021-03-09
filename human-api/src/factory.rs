use crate::*;

use hmt_escrow;
use hmt_escrow::instruction::factory_initialize;
use hmt_escrow::state::Factory;
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_program::instruction::Instruction;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};

#[derive(Serialize, Deserialize)]
pub struct InitFactoryArgs {
    /// Gas payer pub key
    pub gasPayer: String,
    /// solana_sdk::signature::Keypair in Base58 string
    pub gasPayerPrivate: String,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    /// Response data
    pub data: String,
}

///  Returns addresses of all jobs deployed in the factory
#[get("/factory?<_address>")]
pub fn get_factory(_address: String) -> JsonValue {
    unimplemented!();
}

/// Creates a new factory and returns the address
#[post("/factory", format = "json", data = "<init_args>")]
pub fn new_factory(init_args: Json<InitFactoryArgs>, config: State<Config>) -> Json<Response> {
    let payer = Keypair::from_base58_string(&init_args.gasPayerPrivate);

    let factory_acc = Keypair::new();

    let factory_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Factory::LEN)
        .unwrap();

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
        .unwrap(),
    ];

    let signers = vec![&payer, &factory_acc];

    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash().unwrap();

    helpers::check_fee_payer_balance(
        &config,
        &payer.pubkey(),
        factory_account_balance + fee_calculator.calculate_fee(&transaction.message()),
    )
    .unwrap();
    transaction.sign(&signers, recent_blockhash);

    config
        .rpc_client
        .send_and_confirm_transaction(&transaction)
        .unwrap();

    Json(Response {
        data: factory_acc.pubkey().to_string(),
    })
}
