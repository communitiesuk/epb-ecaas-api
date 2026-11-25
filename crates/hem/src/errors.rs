use lambda_http::{Body, Error, Response};
use serde_json::json;
use uuid::Uuid;

pub fn error_x_legacy(
    e: impl std::error::Error,
    status: u16,
    _aws_request_id: Option<String>,
) -> Result<Response<Body>, Error> {
    let error_json =
        json!({"id": Uuid::new_v4(), "status": status.to_string(), "detail": e.to_string()});
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error_json)?))
        .map_err(Box::new)?)
}

pub fn error_415_legacy(
    e: impl std::error::Error,
    aws_request_id: Option<String>,
) -> Result<Response<Body>, Error> {
    error_x_legacy(e, 415, aws_request_id)
}
