use rocket_contrib::json::{Json, JsonValue};
use serde::{Deserialize, Serialize};

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

/// Creates a new job and returns the address
#[post("/job", format = "json", data = "<_job_init_args>")]
pub fn new_job(_job_init_args: Json<InitJobArgs>) -> JsonValue {
    unimplemented!();
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
    data = "<_store_results_args>"
)]
pub fn store_job_intermediate_results(_store_results_args: Json<StoreResultsArgs>) -> JsonValue {
    unimplemented!();
}

/// Performs a payout to multiple Solana addresses
#[post("/bulkPayout", format = "json", data = "<_bulk_payout_args>")]
pub fn bulk_payout(_bulk_payout_args: Json<BulkPayoutArgs>) -> JsonValue {
    unimplemented!();
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
