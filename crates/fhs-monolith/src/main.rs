mod errors;
pub(crate) mod fhs;

use crate::errors::{ApiError, error_415_legacy, response_for_error};
use crate::fhs::FhsMeta;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use home_energy_model_wrapper_fhs::{FhsFlags, SinkOutputWriter, run_wrappers};
use lambda_common::errors::UnsupportedBodyError;
use lambda_common::{create_sentry_guard, extract_aws_request_id, set_tracing_subscriber};
use lambda_http::{Body, Error, Request, Response, run, service_fn};
use resolve_products::resolve_products;
use serde_json::json;
use tokio::sync::OnceCell;
use tracing::error;

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
            return error_415_legacy(
                UnsupportedBodyError::new("Non-text inputs are not accepted"),
                aws_request_id,
            );
        }
    }
    .as_bytes();

    let output = SinkOutputWriter {};

    let input = match resolve_products(input, get_global_client().await).await {
        Ok(input) => input,
        Err(e) => {
            let (response, api_error) =
                response_for_error(Box::new(e) as Box<dyn std::error::Error>, aws_request_id)?;
            if let Some(api_error) = api_error {
                report_api_error(&api_error);
            }

            return Ok(response);
        }
    };

    let resp = match run_wrappers(input, &output, None, None, &FhsFlags::FHS_COMPLIANCE, false, false, false, &[]) {
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
        Err(e) => {
            let (response, api_error) = response_for_error(
                Box::new(e) as Box<dyn std::error::Error>,
                aws_request_id,
            )?;
            if let Some(api_error) = api_error {
                report_api_error(&api_error);
            }

            response
        }
    };

    Ok(resp)
}

fn main() -> Result<(), Error> {
    set_tracing_subscriber()?;

    let _guard = create_sentry_guard(ERROR_REPORTING_APP_PREFIX);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { run(service_fn(function_handler)).await })?;

    Ok(())
}

const ERROR_REPORTING_APP_PREFIX: &str = "ecaas-fhs-api";

fn report_api_error(api_error: &ApiError) {
    error!(
        "{} {} reported with details {:?} and source error {:?}",
        api_error.status(),
        api_error.title().unwrap_or("Unclassified error"),
        api_error.error(),
        api_error.source()
    );
    sentry::capture_error(api_error.error());
}
