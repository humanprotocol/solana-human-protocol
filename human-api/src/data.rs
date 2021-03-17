use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;

#[allow(non_snake_case)]
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

#[derive(Serialize, Deserialize)]
pub struct BoolResponse {
    /// Response data
    pub success: bool,
}

#[derive(Serialize, Deserialize)]
pub struct FactoryJobs {
    /// Response data
    pub jobs: Vec<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct InitJobArgs {
    /// Gas payer pub key
    pub gasPayer: String,
    /// Gas payer private key
    pub gasPayerPrivate: String,
    /// Factory address
    pub factoryAddress: String,
    /// Reputation oracle pub key
    pub repOraclePub: String,
    /// Manifest URL
    pub manifestUrl: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct StoreResultsArgs {
    /// Gas payer pub key
    pub gasPayer: String,
    /// Gas payer private key
    pub gasPayerPrivate: String,
    /// Factory address
    pub address: String,
    /// Reputation oracle pub key
    pub repOraclePub: String,
    /// Result URL
    pub resultsUrl: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct BulkPayoutArgs {
    /// Gas payer pub key
    pub gasPayer: String,
    /// Gas payer private key
    pub gasPayerPrivate: String,
    /// Factory address
    pub address: String,
    /// Reputation oracle pub key
    pub repOraclePub: String,
    /// Result URL
    pub resultsUrl: String,
    /// Payouts URL
    pub payoutsUrl: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct TrustedHandlersArgs {
    /// Gas payer pub key
    pub gasPayer: String,
    /// Gas payer private key
    pub gasPayerPrivate: String,
    /// Factory address
    pub address: String,
    /// List of handlers
    pub handlers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub expiration_date: u8,
    pub instant_result_delivery_webhook: String,
    pub job_mode: String,
    pub job_total_tasks: u64,
    pub minimum_trust_client: f64,
    pub minimum_trust_server: f64,
    pub oracle_stake: f64,
    pub recording_oracle_addr: String,
    pub reputation_agent_addr: String,
    pub reputation_oracle_addr: String,
    pub request_type: String,
    pub requester_accuracy_target: f64,
    pub requester_question: HashMap<String, String>,
    pub requester_question_example: String,
    pub requester_restricted_answer_set: HashMap<String, HashMap<String, String>>,
    pub task_bid_price: f64,
    pub taskdata_uri: String,
    pub unsafe_content: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResultsData {
    pub results: bool,
}

pub struct PayoutRecord {
    pub recipient: Pubkey,
    pub amount: f64,
}
