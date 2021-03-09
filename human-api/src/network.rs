use rocket_contrib::json::JsonValue;

#[get("/stats")]
pub fn get_network_stats() -> JsonValue {
    unimplemented!();
}

#[get("/all")]
pub fn get_networks() -> JsonValue {
    unimplemented!();
}