use aws_sdk_xray as xray;
use chrono::NaiveDate;
use hem::errors::HemError;
use hem::output::{Output, SinkOutput};
use hem::read_weather_file::weather_data_to_vec;
use hem::{
    run_project, ProjectFlags, FHS_VERSION, FHS_VERSION_DATE, HEM_VERSION, HEM_VERSION_DATE,
};
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use parking_lot::Mutex;
use sentry::ClientOptions;
use serde::Serialize;
use serde_json::json;
use std::error::Error as StdError;
use std::io;
use std::io::{BufReader, Cursor, ErrorKind, Write};
use std::str::from_utf8;
use std::sync::Arc;
use tracing_subscriber::fmt::format::FmtSpan;
use uuid::Uuid;

async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Extract some useful information from the request
    let input = match event.body() {
        Body::Empty => "",
        Body::Text(text) => text.as_str(),
        Body::Binary(_) => unimplemented!(),
    }
    .as_bytes();

    let output = SinkOutput {};

    let external_conditions =
        weather_data_to_vec(BufReader::new(Cursor::new(include_str!("./weather.epw")))).ok();

    let resp = match run_project(input, output, external_conditions, &ProjectFlags::FHS_COMPLIANCE) {
        Ok(Some(resp)) => Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({"data": resp, "meta": FhsMeta::default()}))?))
            .map_err(Box::new)?,
        Ok(None) => Response::builder()
            .status(503)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({"errors": [{"status": "503", "detail": "Calculation response not available"}], "meta": FhsMeta::default()}))?))
            .map_err(Box::new)?,
        Err(e @ HemError::InvalidRequest(_)) => error_422(e)?,
        Err(e @ HemError::PanicInWrapper(_)) => error_500(e)?,
        Err(e @ HemError::FailureInCalculation(_)) => error_500(e)?,
        Err(e @ HemError::PanicInCalculation(_)) => error_500(e)?,
        Err(e @ HemError::ErrorInPostprocessing(_)) => error_500(e)?,
        Err(e @ HemError::GeneralPanic(_)) => {
            error_500(e)?
        },
    };

    Ok(resp)
}

#[tokio::main]
async fn send_x_ray_traces() {
    {
        let config = aws_config::load_from_env().await;
        let xray_client = xray::Client::new(&config);

        let trace_segment = json!({
        "name" : "Test trace segment",
        "id" : "70de5b6f19ff9a0b",
        "start_time" : 1.478293361271E9,
        "trace_id" : "1-581cf771-a006649127e371903a2de979",
        "in_progress": true
        })
        .to_string();

        let xray_builder = xray_client
            .put_trace_segments()
            .set_trace_segment_documents(Some(vec![trace_segment]));

        xray_builder.send().await;
    }
}

fn main() -> Result<(), Error> {
    send_x_ray_traces();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_span_events(FmtSpan::CLOSE)
            .finish(),
    )?;

    let _guard = match std::env::var("SENTRY_DSN") {
        Ok(dsn) => Some(sentry::init((
            dsn,
            ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))),
        Err(_) => {
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

fn error_422<E>(e: E) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 422)
}

fn error_500<E>(e: E) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    error_x(e, 500)
}

fn error_x<E>(e: E, status: u16) -> Result<Response<Body>, Error>
where
    E: StdError,
{
    Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&json!({"errors": [{"id": Uuid::new_v4(), "status": status.to_string(), "detail": e.to_string()}], "meta": FhsMeta::default()}))?))
            .map_err(Box::new)?)
}

/// This output uses a shared string that individual "file" writers (the FileLikeStringWriter type)
/// can write to - this string can then be used as the response body for the Lambda.
#[derive(Debug)]
#[allow(dead_code)]
struct LambdaOutput(Arc<Mutex<String>>);

impl LambdaOutput {
    #[allow(dead_code)]
    fn new() -> Self {
        Self(Arc::new(Mutex::new(String::with_capacity(
            // output is expected to be about 4MB so allocate this up front
            2usize.pow(22),
        ))))
    }
}

impl Output for LambdaOutput {
    fn writer_for_location_key(&self, location_key: &str) -> anyhow::Result<impl Write> {
        Ok(FileLikeStringWriter::new(
            self.0.clone(),
            location_key.to_string(),
        ))
    }
}

impl Output for &LambdaOutput {
    fn writer_for_location_key(&self, location_key: &str) -> anyhow::Result<impl Write> {
        <LambdaOutput as Output>::writer_for_location_key(self, location_key)
    }
}

impl From<LambdaOutput> for Body {
    fn from(value: LambdaOutput) -> Self {
        Arc::try_unwrap(value.0).unwrap().into_inner().into()
    }
}

/// Represents a writer for an individual "file".
struct FileLikeStringWriter {
    string: Arc<Mutex<String>>,
    location_key: String,
    has_output_file_header: bool,
}

impl FileLikeStringWriter {
    fn new(string: Arc<Mutex<String>>, location_key: String) -> Self {
        Self {
            string,
            location_key,
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
            output_string
                .push_str(format!("Writing out file '{}':\n\n", self.location_key).as_str());
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

/// Metadata object containing versioning information for the HEM calculation. Corresponds to "FhsMeta" in the API specification.
#[derive(Serialize)]
struct FhsMeta {
    hem_version: &'static str,
    hem_version_date: NaiveDate,
    fhs_version: &'static str,
    fhs_version_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    software_version: Option<&'static str>,
}

impl Default for FhsMeta {
    fn default() -> Self {
        Self {
            hem_version: HEM_VERSION,
            hem_version_date: NaiveDate::parse_from_str(HEM_VERSION_DATE, "%Y-%m-%d").unwrap(),
            fhs_version: FHS_VERSION,
            fhs_version_date: NaiveDate::parse_from_str(FHS_VERSION_DATE, "%Y-%m-%d").unwrap(),
            software_version: option_env!("HEM_SOFTWARE_VERSION"),
        }
    }
}
