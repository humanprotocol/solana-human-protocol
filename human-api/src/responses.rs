use rocket::response::Responder;
use rocket_contrib::json::Json;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    /// Response data
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StatusResponse {
    /// Escrow status
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BalanceResponse {
    /// Escrow token balance
    pub data: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BoolResponse {
    /// Response data
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FactoryJobs {
    /// Response data
    pub jobs: Vec<String>,
}

#[derive(Responder, Debug)]
pub enum OkResponse {
    #[response(status = 200, content_type = "json")]
    DataResponse(Json<Response>),
    #[response(status = 200, content_type = "json")]
    StatusResponse(Json<StatusResponse>),
    #[response(status = 200, content_type = "json")]
    BalanceResponse(Json<BalanceResponse>),
    #[response(status = 200, content_type = "json")]
    BoolResponse(Json<BoolResponse>),
    #[response(status = 200, content_type = "json")]
    FactoryJobsResponse(Json<FactoryJobs>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InvalidParameter {
    /// Parameter name
    pub parameter_name: String,
    /// Error message
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorMessage {
    /// Error message
    pub error: String,
}

#[derive(Responder, Debug)]
pub enum ErrorResponse {
    #[response(status = 400, content_type = "json")]
    InvalidParameterResponse(Json<InvalidParameter>),
    #[response(status = 404, content_type = "json")]
    NotFoundResponse(Json<ErrorMessage>),
    #[response(status = 401, content_type = "json")]
    UnauthorizedResponse(Json<ErrorMessage>),
    #[response(status = 500, content_type = "json")]
    ServerErrorResponse(Json<ErrorMessage>),
    #[response(status = 502, content_type = "json")]
    BadGatewayErrorResponse(Json<ErrorMessage>),
}
