pub mod errors;

use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayProxyRequestContext, ApiGatewayV2httpRequestContext,
};
use lambda_http::request::RequestContext;
use lambda_http::{Request, RequestExt};
use sentry::{ClientInitGuard, ClientOptions};
use std::borrow::Cow;
use tracing::dispatcher::SetGlobalDefaultError;
use tracing_subscriber::fmt::format::FmtSpan;

pub fn create_sentry_guard(app_prefix: &str) -> Option<ClientInitGuard> {
    match option_env!("SENTRY_DSN_API") {
        Some(dsn) => Some(sentry::init((
            dsn,
            ClientOptions::new()
                .maybe_release(sentry::release_name!())
                .environment(
                    std::env::var("SENTRY_ENVIRONMENT")
                        .map(|env| Cow::Owned(format!("{app_prefix}-{env}")))
                        .unwrap_or(Cow::Owned(format!("{app_prefix}-unknown"))),
                ),
        ))),
        None => {
            tracing::warn!("Sentry DSN is not set up in this environment.");
            None
        }
    }
}

pub fn set_tracing_subscriber() -> Result<(), SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_span_events(FmtSpan::CLOSE)
            .finish(),
    )
}

pub fn extract_aws_request_id(event: &Request) -> Option<String> {
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
