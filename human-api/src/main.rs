#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

pub mod factory;
pub mod job;
pub mod manifest;

pub use crate::factory::*;
pub use crate::job::*;
pub use crate::manifest::*;

fn main() {
    rocket::ignite()
        .mount("/factory", routes![get_factory, new_factory])
        .mount("/job", routes![new_job])
        .mount("/manifest", routes![validate_manifest])
        .launch();
}
