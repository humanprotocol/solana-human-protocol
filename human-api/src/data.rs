use serde::{Deserialize, Serialize};

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
pub struct FactoryJobs {
    /// Response data
    pub jobs: Vec<String>,
}

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

// FIXME
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    /// Oracle stake
    pub oracle_stake: u8,
    /// Reputation oracle address
    pub reputation_oracle_addr: String,
    /// Recording oracle address
    pub recording_oracle_addr: String,
}
