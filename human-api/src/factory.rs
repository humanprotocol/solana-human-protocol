use crate::*;

use crate::data::*;
use hmt_escrow;
use hmt_escrow::instruction::factory_initialize;
use hmt_escrow::state::Factory;
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};
use serde::{Deserialize, Serialize};
use solana_program::instruction::Instruction;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};

///  Returns addresses of all jobs deployed in the factory
#[get("/factory?<address>")]
pub fn get_factory(address: String, config: State<Config>) -> Json<FactoryJobs> {
    let human_protocol_program = Pubkey::from_str(&config.human_protocol_program).unwrap();

    let memcp = Memcmp {
        offset: config.offset,
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
        .unwrap();

    Json(FactoryJobs {
        jobs: accounts_with_config
            .iter()
            .map(|account_data| account_data.0.to_string())
            .collect::<Vec<_>>(),
    })
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
