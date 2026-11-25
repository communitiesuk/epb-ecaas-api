use lambda_http::{run, service_fn, Error};
mod errors;
mod http_handler;

use http_handler::function_handler;
use lambda_common::set_tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Error> {
    set_tracing_subscriber()?;

    run(service_fn(function_handler)).await
}
