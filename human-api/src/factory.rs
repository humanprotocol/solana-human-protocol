use rocket_contrib::json::JsonValue;

#[get("/factory?<_program_address>&<_factory_address>")]
pub fn factory_jobs(_program_address: String, _factory_address: String) -> JsonValue {
    unimplemented!();
}

#[post("/factory")]
pub fn create_factory() -> JsonValue {
    unimplemented!();
}