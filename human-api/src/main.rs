#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

pub mod factory;
pub mod helpers;
pub mod job;
pub mod manifest;

pub use crate::factory::*;
pub use crate::helpers::*;
pub use crate::job::*;
pub use crate::manifest::*;

use rocket::fairing::AdHoc;
use solana_client::rpc_client::RpcClient;

pub struct Config {
    pub factory_version: u8,
    pub rpc_client: RpcClient,
}

pub type Error = Box<dyn std::error::Error>;

fn main() {
    rocket::ignite()
        .attach(AdHoc::on_attach("Solana client config", |rocket| {
            let rpc_endpoint = rocket
                .config()
                .get_str("node_endpoint")
                .unwrap()
                .to_string();
            let factory_version = rocket.config().get_int("factory_version").unwrap_or(1) as u8;
            let rpc_client = RpcClient::new(rpc_endpoint);

            Ok(rocket.manage(Config {
                factory_version,
                rpc_client,
            }))
        }))
        .mount("/factory", routes![get_factory, new_factory])
        .mount("/job", routes![new_job])
        .mount("/manifest", routes![validate_manifest])
        .launch();
}
