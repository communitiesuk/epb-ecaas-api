use chrono::NaiveDate;
use home_energy_model_wrapper_fhs::{FHS_VERSION, FHS_VERSION_DATE, HEM_VERSION, HEM_VERSION_DATE};
use serde::Serialize;

/// Metadata object containing versioning information for the HEM calculation, and a request ID. Corresponds to "FhsMeta" in the API specification.
#[derive(Serialize)]
pub struct FhsMeta {
    hem_version: &'static str,
    hem_version_date: NaiveDate,
    fhs_version: &'static str,
    fhs_version_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    software_version: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecaas_request_id: Option<String>,
}

impl Default for FhsMeta {
    fn default() -> Self {
        Self {
            hem_version: HEM_VERSION,
            hem_version_date: NaiveDate::parse_from_str(HEM_VERSION_DATE, "%Y-%m-%d").unwrap(),
            fhs_version: FHS_VERSION,
            fhs_version_date: NaiveDate::parse_from_str(FHS_VERSION_DATE, "%Y-%m-%d").unwrap(),
            software_version: option_env!("HEM_SOFTWARE_VERSION"),
            ecaas_request_id: None,
        }
    }
}

impl FhsMeta {
    pub fn with_request_id(request_id: Option<String>) -> Self {
        Self {
            ecaas_request_id: request_id,
            ..Default::default()
        }
    }
}
