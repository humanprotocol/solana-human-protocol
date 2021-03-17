#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

pub mod data;
pub mod factory;
pub mod helpers;
pub mod job;
pub mod manifest;

pub use crate::factory::*;
// pub use crate::helpers::*;
pub use crate::job::*;
pub use crate::manifest::*;

use rocket::fairing::AdHoc;
use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::Memcmp;
use solana_client::rpc_filter::MemcmpEncodedBytes;
use solana_client::rpc_filter::RpcFilterType;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub struct Config {
    pub factory_version: u8,
    pub token_decimals: u8,
    pub rpc_client: RpcClient,
    pub human_protocol_program: String,
    pub data_offset_to_begin_match: usize,
    pub escrow_duration: u64,
}

pub type Error = Box<dyn std::error::Error>;

/// Check if API is alive
#[get("/ping")]
pub fn ping() -> String {
    String::from("pong")
}

pub fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .attach(AdHoc::on_attach("Solana client config", |rocket| {
            let rpc_endpoint = rocket
                .config()
                .get_str("node_endpoint")
                .unwrap()
                .to_string();
            let rpc_client = RpcClient::new(rpc_endpoint);
            let factory_version = rocket.config().get_int("factory_version").unwrap_or(1) as u8;
            let human_protocol_program =
                String::from(rocket.config().get_str("human_protocol_program").unwrap());
            let data_offset_to_begin_match = rocket
                .config()
                .get_int("data_offset_to_begin_match")
                .unwrap() as usize;
            let escrow_duration = rocket.config().get_int("escrow_duration").unwrap() as u64;
            let token_decimals = rocket.config().get_int("token_decimals").unwrap() as u8;
            let config = Config {
                factory_version,
                token_decimals,
                rpc_client,
                human_protocol_program,
                data_offset_to_begin_match,
                escrow_duration,
            };

            Ok(rocket.manage(config))
        }))
        .mount("/", routes![get_factory, new_factory])
        .mount("/job", routes![new_job])
        .mount("/manifest", routes![validate_manifest])
        .mount("/", routes![ping])
}

fn main() {
    rocket().launch();
}

#[cfg(test)]
mod test {
    use super::*;
    use rocket::http::Status;
    use rocket::local::Client;
    use serde_json::{json, Value};
    use solana_account_decoder::UiAccount;
    use solana_account_decoder::UiAccountEncoding;
    use solana_client::mock_sender::Mocks;
    use solana_client::rpc_request::RpcRequest;
    use solana_client::rpc_response::RpcKeyedAccount;
    use solana_sdk::{account::Account, pubkey::Pubkey};
    use std::collections::HashMap;

    pub const TEST_ENDPOINT: &str = "TestUrl";
    pub const FACTORY_VERSION: u8 = 1;
    pub const TOKEN_DECIMALS: u8 = 9;
    pub const HUMAN_PROTOCOL_PROGRAM: &str = "rK6j1hcHDTWerdrAS2w3BFifjHkPrRrnGYC7GRNwqKF";
    pub const OFFSET: usize = 348;
    pub const DURATION: u64 = 3400;

    pub struct MockedRpcClient {
        solana_client: Option<RpcClient>,
        mocks: Mocks,
    }

    impl MockedRpcClient {
        pub fn new() -> Self {
            Self {
                solana_client: None,
                mocks: HashMap::new(),
            }
        }

        pub fn create_rpc_client(&mut self) {
            self.solana_client = Some(RpcClient::new_mock_with_mocks(
                String::from(TEST_ENDPOINT),
                self.mocks.clone(),
            ));
        }

        pub fn mock_get_program_addresses_with_filter(&mut self) -> Pubkey {
            let p_k = Pubkey::new_unique();
            let account = Account::new(10, 10, &Pubkey::new_unique());
            let ui_account =
                UiAccount::encode(&p_k, account, UiAccountEncoding::Base64, None, None);
            let response_data = vec![RpcKeyedAccount {
                pubkey: p_k.to_string(),
                account: ui_account,
            }];
            self.mocks
                .insert(RpcRequest::GetProgramAccounts, json!(response_data));
            p_k
        }
    }

    fn test_rocket(mocked_client: MockedRpcClient) -> rocket::Rocket {
        rocket::ignite()
            .attach(AdHoc::on_attach("Solana client config", |rocket| {
                let config = Config {
                    factory_version: FACTORY_VERSION,
                    token_decimals: TOKEN_DECIMALS,
                    rpc_client: mocked_client.solana_client.unwrap(),
                    human_protocol_program: String::from(HUMAN_PROTOCOL_PROGRAM),
                    data_offset_to_begin_match: OFFSET,
                    escrow_duration: DURATION,
                };

                Ok(rocket.manage(config))
            }))
            .mount("/", routes![get_factory, new_factory])
            .mount("/job", routes![new_job])
            .mount("/manifest", routes![validate_manifest])
            .mount("/", routes![ping])
    }

    #[test]
    fn test_get_factory_addresses() {
        let mut rpc_client = MockedRpcClient::new();
        let response_pub_key = rpc_client.mock_get_program_addresses_with_filter();
        rpc_client.create_rpc_client();

        let client = Client::new(test_rocket(rpc_client)).expect("valid rocket instance");
        let mut response = client
            .get(format!("/factory?address={}", HUMAN_PROTOCOL_PROGRAM))
            .dispatch();
        assert_eq!(response.status(), Status::Ok);

        let response_body: Value = serde_json::from_str(&response.body_string().unwrap()).unwrap();
        let expected_response = Value::Array(vec![Value::String(response_pub_key.to_string())]);
        assert_eq!(expected_response, response_body["jobs"]);
    }
}
