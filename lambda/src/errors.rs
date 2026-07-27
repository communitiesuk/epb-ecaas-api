use crate::fhs::FhsMeta;
use lambda_http::{Body, Error, Response};
use serde::de::StdError;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

pub fn error_415<E>(e: E, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 415, aws_request_id)
}

pub fn error_422<E>(e: E, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 422, aws_request_id)
}

pub fn error_500<E>(e: E, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 500, aws_request_id)
}

pub fn error_x<E>(
    e: E,
    status: u16,
    aws_request_id: Option<String>,
) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&json!({"errors": [{"id": Uuid::new_v4(), "status": status.to_string(), "detail": e.to_string()}], "meta": FhsMeta::with_request_id(aws_request_id)}))?))
        .map_err(Box::new)?)
}

#[derive(Debug, Error)]
#[error("Error resolving products from PCDB: {0}")]
pub struct ResolveProductError(String);

impl ResolveProductError {
    pub fn new<T: Into<String>>(message: T) -> Self {
        Self(message.into())
    }
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct UnsupportedBodyError(&'static str);

impl UnsupportedBodyError {
    pub fn new(message: &'static str) -> Self {
        Self(message)
    }
}
