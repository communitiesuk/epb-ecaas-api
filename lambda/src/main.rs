use chrono::NaiveDate;
use hem::errors::HemError;
use hem::output::{Output, SinkOutput};
use hem::read_weather_file::weather_data_to_vec;
use hem::{
    run_project, ProjectFlags, FHS_VERSION, FHS_VERSION_DATE, HEM_VERSION, HEM_VERSION_DATE,
};
use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayProxyRequestContext, ApiGatewayV2httpRequestContext,
};
use lambda_http::request::RequestContext;
use lambda_http::{run, service_fn, Body, Error, Request, RequestExt, Response};
use parking_lot::Mutex;
use resolve_products::errors::ResolvePcdbProductsError;
use sentry::ClientOptions;
use serde::Serialize;
use serde_json::json;
use std::error::Error as StdError;
use std::io;
use std::io::{BufReader, Cursor, ErrorKind, Write};
use std::str::from_utf8;
use std::sync::Arc;
use resolve_products::resolve_products;
use thiserror::Error;
use tracing::error;
use tracing_subscriber::fmt::format::FmtSpan;
use uuid::Uuid;

async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Extract some useful information from the request
    let aws_request_id = extract_aws_request_id(&event);
    let input = match event.body() {
        Body::Empty => "",
        Body::Text(text) => text.as_str(),
        _ => return error_415(UnsupportedBodyError("Non-text inputs are not accepted"), aws_request_id),
    }
    .as_bytes();

    let output = SinkOutput {};

    let external_conditions =
        weather_data_to_vec(BufReader::new(Cursor::new(include_str!("./weather.epw")))).ok();

    let input = match resolve_products(input) {
        Ok(input) => input,
        Err(e) => {
            return if let ResolvePcdbProductsError::InvalidRequest(_) = e {
                // don't wrap in the resolve product error because this is just down to the request being invalid
                error_422(e, aws_request_id)
            } else {
                error_422(ResolveProductError(e.to_string()), aws_request_id)
            };
        }
    };

    let resp = match run_project(input, output, external_conditions, None, &ProjectFlags::FHS_COMPLIANCE) {
        Ok(Some(resp)) => Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({"data": resp, "meta": FhsMeta::with_request_id(aws_request_id)}))?))
            .map_err(Box::new)?,
        Ok(None) => Response::builder()
            .status(503)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({"errors": [{"status": "503", "detail": "Calculation response not available"}], "meta": FhsMeta::with_request_id(aws_request_id)}))?))
            .map_err(Box::new)?,
        Err(e @ HemError::InvalidRequest(_)) => error_422(e, aws_request_id)?,
        Err(e @ HemError::PanicInWrapper(_)) => {
            let response = error_500(&e, aws_request_id);
            error!("{:?}", e);
            response?
        },
        Err(e @ HemError::FailureInCalculation(_)) => {
            let response = error_500(&e, aws_request_id);
            error!("{:?}", e);
            response?
        },
        Err(e @ HemError::PanicInCalculation(_)) => {
            let response = error_500(&e, aws_request_id);
            error!("{:?}", e);
            response?
        },
        Err(e @ HemError::ErrorInPostprocessing(_)) => {
            let response = error_500(&e, aws_request_id);
            error!("{:?}", e);
            response?
        },

        Err(e @ HemError::NotImplemented(_)) => {
            let response = error_x(&e, 501, aws_request_id);
            error!("{:?}", e);
            response?
        }
        Err(e) => {
            let response = error_500(&e, aws_request_id);
            error!("{:?}", e);
            response?
        },
    };

    Ok(resp)
}

fn main() -> Result<(), Error> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_span_events(FmtSpan::CLOSE)
            .finish(),
    )?;

    let _guard = match option_env!("SENTRY_DSN") {
        Some(dsn) => Some(sentry::init((
            dsn,
            ClientOptions {
                release: sentry::release_name!(),
                environment: Some(option_env!("SENTRY_ENVIRONMENT").unwrap_or("development").into()),
                ..Default::default()
            },
        ))),
        None => {
            tracing::warn!("Sentry DSN is not set up in this environment.");
            None
        }
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { run(service_fn(function_handler)).await })?;

    Ok(())
}

fn error_415<E>(e: E, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 415, aws_request_id)
}

fn error_422<E>(e: E, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 422, aws_request_id)
}

fn error_500<E>(e: E, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 500, aws_request_id)
}

fn error_x<E>(e: E, status: u16, aws_request_id: Option<String>) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({"errors": [{"id": Uuid::new_v4(), "status": status.to_string(), "detail": e.to_string()}], "meta": FhsMeta::with_request_id(aws_request_id)}))?))
            .map_err(Box::new)?)
}

fn extract_aws_request_id(event: &Request) -> Option<String> {
    match event.extensions().request_context() {
        RequestContext::ApiGatewayV2(ApiGatewayV2httpRequestContext { request_id, .. }) => {
            request_id
        }
        RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext { request_id, .. }) => {
            request_id
        }
        _ => None
    }
}

#[derive(Debug, Error)]
#[error("Error resolving products from PCDB: {0}")]
struct ResolveProductError(String);

#[derive(Debug, Error)]
#[error("{0}")]
struct UnsupportedBodyError(&'static str);

/// This output uses a shared string that individual "file" writers (the FileLikeStringWriter type)
/// can write to - this string can then be used as the response body for the Lambda.
#[derive(Debug)]
#[allow(dead_code)]
struct LambdaOutput(Arc<Mutex<String>>);

impl LambdaOutput {
    #[allow(dead_code)]
    fn new() -> Self {
        Self(Arc::new(Mutex::new(String::with_capacity(
            // output is expected to be about 4MB so allocate this up front
            2usize.pow(22),
        ))))
    }
}

impl Output for LambdaOutput {
    fn writer_for_location_key(
        &self,
        location_key: &str,
        file_extension: &str,
    ) -> anyhow::Result<impl Write> {
        Ok(FileLikeStringWriter::new(
            self.0.clone(),
            location_key.to_string(),
            file_extension.to_string(),
        ))
    }
}

impl Output for &LambdaOutput {
    fn writer_for_location_key(
        &self,
        location_key: &str,
        file_extension: &str,
    ) -> anyhow::Result<impl Write> {
        <LambdaOutput as Output>::writer_for_location_key(self, location_key, file_extension)
    }
}

impl From<LambdaOutput> for Body {
    fn from(value: LambdaOutput) -> Self {
        Arc::try_unwrap(value.0).unwrap().into_inner().into()
    }
}

/// Represents a writer for an individual "file".
struct FileLikeStringWriter {
    string: Arc<Mutex<String>>,
    location_key: String,
    file_extension: String,
    has_output_file_header: bool,
}

impl FileLikeStringWriter {
    fn new(string: Arc<Mutex<String>>, location_key: String, file_extension: String) -> Self {
        Self {
            string,
            location_key,
            file_extension,
            has_output_file_header: false,
        }
    }
}

impl Write for FileLikeStringWriter {
    /// Writes out bytes to this "file" (part of the wider LambdaOutput string), making sure there is
    /// a human-readable header at the start of the file so a human can know what each part of the output
    /// is sourced from.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.has_output_file_header {
            let mut output_string = self.string.lock();
            if !output_string.is_empty() {
                output_string.push_str("\n\n");
            }
            output_string.push_str(
                format!(
                    "Writing out file '{}' ({}):\n\n",
                    self.location_key, self.file_extension
                )
                .as_str(),
            );
            self.has_output_file_header = true;
        }
        let utf8 = match from_utf8(buf) {
            Ok(utf8) => utf8,
            Err(_) => {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "Tried to write out invalid UTF-8.",
                ));
            }
        };
        self.string.lock().push_str(utf8);
        Ok(utf8.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Metadata object containing versioning information for the HEM calculation, and a request ID. Corresponds to "FhsMeta" in the API specification.
#[derive(Serialize)]
struct FhsMeta {
    hem_version: &'static str,
    hem_version_date: NaiveDate,
    fhs_version: &'static str,
    fhs_version_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    software_version: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecaas_request_id: Option<String>,
}

impl Default for FhsMeta {
    fn default() -> Self {
        Self {
            hem_version: HEM_VERSION,
            hem_version_date: NaiveDate::parse_from_str(HEM_VERSION_DATE, "%Y-%m-%d").unwrap(),
            fhs_version: FHS_VERSION,
            fhs_version_date: NaiveDate::parse_from_str(FHS_VERSION_DATE, "%Y-%m-%d").unwrap(),
            software_version: option_env!("HEM_SOFTWARE_VERSION"),
            ecaas_request_id: None,
        }
    }
}

impl FhsMeta {
    fn with_request_id(request_id: Option<String>) -> Self {
        Self {
            ecaas_request_id: request_id,
            ..Default::default()
        }
    }
}
