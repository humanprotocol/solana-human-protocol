use rocket_contrib::json::JsonValue;

#[get("/validate?<_manifest_url>")]
pub fn validate_manifest(_manifest_url: String) -> JsonValue {
    unimplemented!();
}