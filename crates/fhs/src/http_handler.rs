use aws_sdk_sfn::Client as AwsStepFunctionClient;
use ecaas_api_fhs_common::FhsMeta;
use ecaas_api_fhs_common::dynamodb::get_global_dynamodb_client;
use ecaas_api_fhs_common::errors::{error_415_legacy, report_api_error, response_for_error};
use ecaas_api_lambda_common::errors::UnsupportedBodyError;
use ecaas_api_lambda_common::extract_aws_request_id;
use home_energy_model_wrapper_fhs::{
    AwsStepFunctionHemRunner, FhsFlags, SinkOutputWriter, run_wrappers,
};
use lambda_http::{Body, Error, Request, Response};
use resolve_products::resolve_products;
use serde_json::json;

pub async fn function_handler(
    event: Request,
    step_function_arn: &str,
) -> Result<Response<Body>, Error> {
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

    let output_writer = SinkOutputWriter {};

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

    let aws_config = aws_config::load_from_env().await;

    let sfn_client = AwsStepFunctionClient::new(&aws_config);

    let step_function_runner = AwsStepFunctionHemRunner::new(step_function_arn.into(), &sfn_client);

    let resp = match run_wrappers(input, &output_writer, &step_function_runner, None, None, &FhsFlags::FHS_COMPLIANCE, false, false, false, &[]).await {
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
