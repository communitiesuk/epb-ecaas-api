use ecaas_api_fhs_common::FhsMeta;
use ecaas_api_fhs_common::dynamodb::get_global_dynamodb_client;
use ecaas_api_fhs_common::errors::{
    ERROR_REPORTING_APP_PREFIX, error_415_legacy, report_api_error, response_for_error,
};
use ecaas_api_lambda_common::errors::UnsupportedBodyError;
use ecaas_api_lambda_common::{
    create_sentry_guard, extract_aws_request_id, set_tracing_subscriber,
};
use home_energy_model_wrapper_fhs::{FhsFlags, SinkOutputWriter, run_wrappers};
use lambda_http::{Body, Error, Request, Response, run, service_fn};
use resolve_products::resolve_products;
use serde_json::json;

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

    let input = match resolve_products(input, get_global_dynamodb_client().await).await {
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
