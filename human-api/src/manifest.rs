use rocket_contrib::json::JsonValue;

#[allow(non_snake_case, unused_variables)]
#[get("/validate?<manifestUrl>")]
pub fn validate_manifest(manifestUrl: String) -> JsonValue {
    unimplemented!();
}
