use rocket_contrib::json::JsonValue;
use rocket::request::Form;

#[derive(FromForm)]
pub struct InitJobArgs {
    // INITIALIZE DATA
    /// Escrow duration in seconds, escrow can only be canceled after its duration expires
    pub duration: u64,
    /// Mint
    pub mint: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub launcher: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub canceler: String,
    /// Canceler's token account
    pub cancelers_token_acc: String,

    // SETUP DATA
    /// Reputation oracle fee in percents
    pub reputation_oracle_stake: u8,
    /// Recording oracle fee in percents
    pub recording_oracle_stake: u8,
    /// Manifest URL
    pub manifest_url: String,
    /// Manifest hash
    pub manifest_hash: String,
    /// Trusted handler Keypair
    pub trusted_handler: String,
    /// Reputation oracle
    pub reputation_oracle: String,
    /// Reputation's oracle token account
    pub reputations_oracle_token_acc: String,
    /// Recording oracle account
    pub recording_oracle_acc: String,
    /// Recording oracle's token account
    pub recording_oracles_token_acc: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub payer: String,
}

#[derive(FromForm)]
pub struct CancelJobArgs {
    pub job_account: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub trusted_handler: String,
    pub job_token_sending_acc: String,
    pub job_signing_authority: String,
    pub canceler_token_acc: String,
    pub token_program: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub payer: String,
}

#[derive(FromForm)]
pub struct CompleteJobArgs {
    pub job_account: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub trusted_handler: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub payer: String,
}

#[derive(FromForm)]
pub struct StoreResultsArgs {
    /// Total amount to pay
    pub total_amount: u64,
    /// Total number of recipients
    pub total_recipients: u64,
    /// Final results URL
    pub final_results_url: String,
    /// Final results hash
    pub final_results_hash: String,
    pub job_account: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub trusted_handler: String,
    /// solana_sdk::signature::Keypair as Base58 string
    pub payer: String,
}

/// Creates a new job and returns the address
#[post("/job", data="<_job_init_args>")]
pub fn new_job(_job_init_args: Form<InitJobArgs>) -> JsonValue {
    unimplemented!();
}

/// Receive the address of the launcher of a given job address
#[get("/launcher?<_job_address>")]
pub fn get_job_launcher(_job_address: String) -> JsonValue {
    unimplemented!();
}

/// Receive the status of a given job address
#[get("/status?<_job_address>")]
pub fn get_job_status(_job_address: String) -> JsonValue {
    unimplemented!();
}

/// Receive the Manifest URL of a given job address
#[get("/manifestUrl?<_job_address>")]
pub fn get_job_manifest_url(_job_address: String) -> JsonValue {
    unimplemented!();
}

/// Receive the Manifest Hash of a given job address
#[get("/manifestHash?<_job_address>")]
pub fn get_job_manifest_hash(_job_address: String) -> JsonValue {
    unimplemented!();
}

/// Balance in HMT of a given job address
#[get("/balance?<_job_address>")]
pub fn get_job_balance(_job_address: String) -> JsonValue {
    unimplemented!();
}

/// Abort a given job
#[post("/abort", data="<_job_cancel_args>")]
pub fn abort_job(_job_cancel_args: Form<CancelJobArgs>) -> JsonValue {
    unimplemented!();
}

/// Cancel a given job
#[post("/cancel", data="<_job_cancel_args>")]
pub fn cancel_job(_job_cancel_args: Form<CancelJobArgs>) -> JsonValue {
    unimplemented!();
}

/// Complete a given job
#[post("/complete", data="<_job_complete_args>")]
pub fn complete_job(_job_complete_args: Form<CompleteJobArgs>) -> JsonValue {
    unimplemented!();
}

/// Store job results
#[post("/storeIntermediateResults", data="<_store_results_args>")]
pub fn store_job_intermediate_results(_store_results_args: Form<StoreResultsArgs>) -> JsonValue {
    unimplemented!();
}