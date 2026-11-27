use ecaas_api_lambda_common::set_tracing_subscriber;
use http_handler::function_handler;
use lambda_http::{Error, run, service_fn};

mod errors;
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    set_tracing_subscriber()?;

    run(service_fn(function_handler)).await
}
