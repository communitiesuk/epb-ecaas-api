use fhs_common::errors::ERROR_REPORTING_APP_PREFIX;
use lambda_http::{Error, run, service_fn};
mod http_handler;
use http_handler::function_handler;
use lambda_common::{create_sentry_guard, set_tracing_subscriber};

#[tokio::main]
async fn main() -> Result<(), Error> {
    set_tracing_subscriber()?;

    let _guard = create_sentry_guard(ERROR_REPORTING_APP_PREFIX);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { run(service_fn(function_handler)).await })?;

    Ok(())
}
