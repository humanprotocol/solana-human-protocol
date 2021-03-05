#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

pub mod factory;

pub use crate::factory::*;

fn main() {
    rocket::ignite().mount("/factory", routes![factory_jobs, create_factory]).launch();
}