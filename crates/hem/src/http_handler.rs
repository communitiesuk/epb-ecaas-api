use crate::errors::error_415_legacy;
use flate2::write::GzEncoder;
use flate2::Compression;
use home_energy_model::errors::HemError;
use home_energy_model::output_writer::SinkOutputWriter;
use home_energy_model::{run_project_from_input_file, CalculationResult, RunInput};
use lambda_common::errors::UnsupportedBodyError;
use lambda_common::extract_aws_request_id;
use lambda_http::{Body, Error, Request, Response};
use serde_json::json;
use std::io::BufReader;

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
pub(crate) async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    let aws_request_id = extract_aws_request_id(&event);
    let input = match event.body() {
        Body::Empty => "".as_bytes(),
        Body::Text(text) => text.as_bytes(),
        _ => {
            return error_415_legacy(
                UnsupportedBodyError::new("Non-text inputs are not accepted"),
                aws_request_id,
            )
        }
    };

    let output_writer = SinkOutputWriter {};

    match run_project_from_input_file(
        RunInput::Read(Box::new(BufReader::new(input))),
        &output_writer,
        None,
        None,
        None,
        Default::default(),
        Default::default(),
    ) {
        Ok(CalculationResult { output, .. }) => {
            let mut encoder =
                GzEncoder::new(Vec::with_capacity(2 * 1024 * 1024), Compression::fast());
            serde_json::to_writer_pretty(&mut encoder, &output)?;
            let compressed = encoder.finish()?;

            Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .header("content-encoding", "gzip")
                .body(Body::Binary(compressed))?)
        }
        Err(err) => {
            let status = if let HemError::InvalidRequest(_) = &err {
                422
            } else {
                500
            };

            Ok(Response::builder().status(status).body(Body::Text(
                serde_json::to_string_pretty(&json!({"error": err.to_string()}))?,
            ))?)
        }
    }
}
