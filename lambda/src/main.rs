mod errors;
pub(crate) mod fhs;

use crate::errors::{
    error_415, error_422, error_500, error_x, ResolveProductError, UnsupportedBodyError,
};
use crate::fhs::FhsMeta;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use constcat::concat;
use home_energy_model_wrapper_fhs::{
    run_wrappers, FhsFlags, HemError, OutputWriter, SinkOutputWriter,
};
use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayProxyRequestContext, ApiGatewayV2httpRequestContext,
};
use lambda_http::request::RequestContext;
use lambda_http::{run, service_fn, Body, Error, Request, RequestExt, Response};
use parking_lot::Mutex;
use resolve_products::errors::ResolvePcdbProductsError;
use resolve_products::resolve_products;
use sentry::ClientOptions;
use serde_json::json;
use std::borrow::Cow;
use std::io;
use std::io::{ErrorKind, Write};
use std::str::from_utf8;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::error;
use tracing_subscriber::fmt::format::FmtSpan;

static DYNAMO_DB_CLIENT: OnceCell<Client> = OnceCell::const_new();

async fn get_global_client() -> &'static Client {
    DYNAMO_DB_CLIENT
        .get_or_init(|| async {
            // if we are running in a cargo-lambda watch environment, use a local DynamoDB instance
            // otherwise we expect to be within real AWS
            if std::env::var("CARGO_HOME").is_ok() {
                let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    // DynamoDB run locally uses port 8000 by default.
                    .endpoint_url("http://localhost:8000")
                    .load()
                    .await;
                let dynamodb_local_config =
                    aws_sdk_dynamodb::config::Builder::from(&config).build();

                aws_sdk_dynamodb::Client::from_conf(dynamodb_local_config)
            } else {
                let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
                Client::new(&config)
            }
        })
        .await
}

async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Extract some useful information from the request
    let aws_request_id = extract_aws_request_id(&event);
    let input = match event.body() {
        Body::Empty => "",
        Body::Text(text) => text.as_str(),
        _ => {
            return error_415(
                UnsupportedBodyError::new("Non-text inputs are not accepted"),
                aws_request_id,
            )
        }
    }
    .as_bytes();

    let output = SinkOutputWriter {};

    let input = match resolve_products(input, get_global_client().await).await {
        Ok(input) => input,
        Err(e) => {
            return match e {
                ResolvePcdbProductsError::InvalidRequest(_) => {
                    // don't wrap in the resolve product error because this is just down to the request being invalid
                    error_422(e, aws_request_id)
                }
                ResolvePcdbProductsError::AccessError(_) => {
                    error_x(ResolveProductError::new("Error encountered when accessing HEM database products. Try again in a few minutes."), 503, aws_request_id)
                }
                ResolvePcdbProductsError::DeserializeError(_) | ResolvePcdbProductsError::InUseFactorEntryMissingError | ResolvePcdbProductsError::InUseFactorsInaccessibleError => {
                    report_error(&e);
                    error_500(ResolveProductError::new("Error encountered when accessing HEM database information"), aws_request_id)
                }
                _ => error_422(ResolveProductError::new(e.to_string()), aws_request_id)
            };
        }
    };

    let resp = match run_wrappers(input, output, None, None, &FhsFlags::FHS_COMPLIANCE, false, false, false, &[]) {
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
            report_error(&e);
            response?
        },
        Err(e @ HemError::FailureInCalculation(_)) => {
            let response = error_500(&e, aws_request_id);
            report_error(&e);
            response?
        },
        Err(e @ HemError::PanicInCalculation(_)) => {
            let response = error_500(&e, aws_request_id);
            report_error(&e);
            response?
        },
        Err(e @ HemError::ErrorInPostprocessing(_)) => {
            let response = error_500(&e, aws_request_id);
            report_error(&e);
            response?
        },

        Err(e @ HemError::NotImplemented(_)) => {
            let response = error_x(&e, 501, aws_request_id);
            report_error(&e);
            response?
        }
        Err(e) => {
            let response = error_500(&e, aws_request_id);
            report_error(&e);
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
                environment: Some(
                    std::env::var("SENTRY_ENVIRONMENT")
                        .map(|env| Cow::Owned(format!("{ERROR_REPORTING_APP_PREFIX}-{env}")))
                        .unwrap_or(Cow::Borrowed(concat!(
                            ERROR_REPORTING_APP_PREFIX,
                            "-unknown"
                        ))),
                ),
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

const ERROR_REPORTING_APP_PREFIX: &str = "ecaas-fhs-api";

fn report_error<T: std::error::Error>(e: &T) {
    error!("{e:?}");
    sentry::capture_error(&e);
}

fn extract_aws_request_id(event: &Request) -> Option<String> {
    match event.extensions().request_context() {
        RequestContext::ApiGatewayV2(ApiGatewayV2httpRequestContext { request_id, .. }) => {
            request_id
        }
        RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext { request_id, .. }) => {
            request_id
        }
        _ => None,
    }
}

/// This output uses a shared string that individual "file" writers (the FileLikeStringWriter type)
/// can write to - this string can then be used as the response body for the Lambda.
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct LambdaOutputWriter(Arc<Mutex<String>>);

impl LambdaOutputWriter {
    #[allow(dead_code)]
    fn new() -> Self {
        Self(Arc::new(Mutex::new(String::with_capacity(
            // output is expected to be about 4MB so allocate this up front
            2usize.pow(22),
        ))))
    }
}

impl OutputWriter for LambdaOutputWriter {
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

impl OutputWriter for &LambdaOutputWriter {
    fn writer_for_location_key(
        &self,
        location_key: &str,
        file_extension: &str,
    ) -> anyhow::Result<impl Write> {
        <LambdaOutputWriter as OutputWriter>::writer_for_location_key(
            self,
            location_key,
            file_extension,
        )
    }
}

impl From<LambdaOutputWriter> for Body {
    fn from(value: LambdaOutputWriter) -> Self {
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
