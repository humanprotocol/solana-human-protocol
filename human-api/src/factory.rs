use rocket_contrib::json::JsonValue;
use rocket::request::Form;

#[derive(FromForm)]
pub struct InitFactoryArgs {
    /// Version
    pub version: u8,
    /// solana_sdk::signature::Keypair in Base58 string
    pub signer: String,
}

///  Returns addresses of all jobs deployed in the factory
#[get("/factory?<_program_address>&<_factory_address>")]
pub fn get_factory(_program_address: String, _factory_address: String) -> JsonValue {
    unimplemented!();
}

/// Creates a new factory and returns the address
#[post("/factory", data="<_init_args>")]
pub fn new_factory(_init_args: Form<InitFactoryArgs>) -> JsonValue {
    unimplemented!();
}