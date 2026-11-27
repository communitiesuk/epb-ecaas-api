use fhs_common::errors::ERROR_REPORTING_APP_PREFIX;
use lambda_http::{Error, run, service_fn};
mod http_handler;
use http_handler::function_handler;
use lambda_common::{create_sentry_guard, set_tracing_subscriber};

#[tokio::main]
async fn main() -> Result<(), Error> {
    set_tracing_subscriber()?;

    let _guard = create_sentry_guard(ERROR_REPORTING_APP_PREFIX);

    let step_function_arn =
        std::env::var("STEP_FUNCTION_ARN").expect("STEP_FUNCTION_ARN must be set");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            run(service_fn(|event| {
                function_handler(event, &step_function_arn)
            }))
            .await
        })?;

    Ok(())
}
