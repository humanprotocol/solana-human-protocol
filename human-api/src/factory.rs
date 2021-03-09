use rocket_contrib::json::{Json, JsonValue};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct InitFactoryArgs {
    /// Gas payer pub key
    pub gasPayer: String,
    /// solana_sdk::signature::Keypair in Base58 string
    pub gasPayerPrivate: String,
}

///  Returns addresses of all jobs deployed in the factory
#[get("/factory?<_address>")]
pub fn get_factory(_address: String) -> JsonValue {
    unimplemented!();
}

/// Creates a new factory and returns the address
#[post("/factory", format="json", data="<_init_args>")]
pub fn new_factory(_init_args: Json<InitFactoryArgs>) -> JsonValue {
    unimplemented!();
}