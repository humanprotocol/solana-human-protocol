use rocket_contrib::json::JsonValue;

#[get("/validate?<_manifestUrl>")]
pub fn validate_manifest(_manifestUrl: String) -> JsonValue {
    unimplemented!();
}
