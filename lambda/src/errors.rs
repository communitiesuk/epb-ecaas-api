//! Error definitions, including codes!
//!
//! There are different groups of errors for the FHS API.
//! Ultimately, some of these groups of errors (for example, the errors from HEM core) could
//! be reused in other endpoints that also use HEM core.
//!
//! PCDB (HEM database) - 6xx error code (skipping 4xx and 5xx as these can be confused with HTTP status codes)
//!
//! 600 - General error in HEM database layer - PCDB related errors without a more specific error defined
//!
//! 601 - HEM database product references found of unexpected category - Product references are of unexpected category for context in which they are used
//! 602 - HEM database product references found of unsupported category - Product references are of unsupported category for context in which they are used
//! 603 - Unknown references found to products in HEM database - At least one product reference provided cannot be found in the PCDB store.
//! 604 - No energy supply provided for fuel used by referenced product in HEM database - A product uses a particular fuel but there are no energy supplies provided associated with that fuel.
//! 605 - Unrecognised sub heat network name found - A sub heat network indicated in a request was not found to exist for the indicated heat network.
//! 606 - Booster heat pump needed for heat network - A fifth generation heat network was referenced but no booster heat pump provided
//! 607 - Invalid combination of input and HEM database product data - Invalid combination of input and HEM database product data

use crate::errors::hem_database::HemDatabaseLayerErrorClassification;
use crate::fhs::FhsMeta;
use enum_assoc::Assoc;
use home_energy_model_wrapper_fhs::HemError;
use lambda_http::{Body, Error, Response, http::StatusCode};
use resolve_products::errors::ResolvePcdbProductsError;
use serde_json::json;
use std::borrow::Cow;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug)]
pub enum FhsApiErrorClassification {
    Legacy(LegacyClassification),
    // Hem(HemError),
    ResolvePcdbProducts(HemDatabaseLayerErrorClassification),
}

#[derive(Assoc, Debug)]
#[func(pub fn status(&self) -> StatusCode)]
pub enum LegacyClassification {
    #[assoc(status = StatusCode::INTERNAL_SERVER_ERROR)]
    ServerError,
    #[assoc(status = StatusCode::UNPROCESSABLE_ENTITY)]
    ClientError,
    // transient server error - i.e. may be due to brief outage in infrastructure
    #[assoc(status = StatusCode::SERVICE_UNAVAILABLE)]
    TransientServerError,
    #[assoc(status = StatusCode::NOT_IMPLEMENTED)]
    NotImplemented,
}

impl FhsApiErrorClassification {
    #[allow(dead_code)]
    fn code(&self) -> Option<u16> {
        // only pcdb errors have been given error codes yet
        if let FhsApiErrorClassification::ResolvePcdbProducts(e) = self {
            e.code().into()
        } else {
            None
        }
    }

    fn title(&self) -> Option<&'static str> {
        // only pcdb errors have been given error titles yet
        if let FhsApiErrorClassification::ResolvePcdbProducts(e) = self {
            e.title().into()
        } else {
            None
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            FhsApiErrorClassification::Legacy(classification) => classification.status(),
            FhsApiErrorClassification::ResolvePcdbProducts(e) => e.status(),
        }
    }

    fn indicates_should_report(&self) -> bool {
        self.status().is_server_error()
    }
}

pub fn error_415_legacy(
    e: impl std::error::Error,
    aws_request_id: Option<String>,
) -> Result<Response<Body>, Error> {
    error_x_legacy(e, 415, aws_request_id)
}

pub fn error_x_legacy(
    e: impl std::error::Error,
    status: u16,
    aws_request_id: Option<String>,
) -> Result<Response<Body>, Error> {
    let error_json =
        json!({"id": Uuid::new_v4(), "status": status.to_string(), "detail": e.to_string()});
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(
            &json!({"errors": [error_json], "meta": FhsMeta::with_request_id(aws_request_id)}),
        )?))
        .map_err(Box::new)?)
}

pub fn response_for_error(
    error: Box<dyn std::error::Error + 'static>,
    aws_request_id: Option<String>,
) -> Result<(Response<Body>, Option<ApiError>), Error> {
    let api_error = ApiError::from(error);
    let error_json = json_api::ErrorObject::from(&api_error);
    Ok((
        Response::builder()
            .status(api_error.status())
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(
                &json!({"errors": [error_json], "meta": FhsMeta::with_request_id(aws_request_id)}),
            )?))
            .map_err(Box::new)?,
        api_error.should_report().then_some(api_error),
    ))
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct UnsupportedBodyError(&'static str);

impl UnsupportedBodyError {
    pub fn new(message: &'static str) -> Self {
        Self(message)
    }
}

pub struct ApiError {
    classification: FhsApiErrorClassification,
    original_error: Box<dyn std::error::Error + 'static>,
}

impl From<Box<dyn std::error::Error>> for ApiError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        Self {
            classification: Self::classification_from_error(error.as_ref()),
            original_error: error,
        }
    }
}

impl ApiError {
    fn classification_from_error(
        error: &(dyn std::error::Error + 'static),
    ) -> FhsApiErrorClassification {
        // pcdb errors
        if let Some(error) = error.downcast_ref::<ResolvePcdbProductsError>() {
            return FhsApiErrorClassification::from(error);
        }

        // HEM errors
        if let Some(hem_error) = error.downcast_ref::<HemError>() {
            return FhsApiErrorClassification::from(hem_error);
        }

        FhsApiErrorClassification::Legacy(LegacyClassification::ServerError)
    }

    pub fn error(&self) -> &(dyn std::error::Error + 'static) {
        self.original_error.as_ref()
    }

    pub fn source(&self) -> &(dyn std::error::Error + 'static) {
        let mut root: &(dyn std::error::Error + 'static) = self.original_error.as_ref();
        while let Some(source) = root.source() {
            root = source;
        }

        root
    }

    pub fn title(&self) -> Option<&str> {
        self.classification.title()
    }

    pub fn should_report(&self) -> bool {
        self.classification.indicates_should_report()
    }

    pub fn status(&self) -> StatusCode {
        self.classification.status()
    }
}

impl<'a> From<&ApiError> for json_api::ErrorObject<'a> {
    fn from(error_with_context: &ApiError) -> Self {
        Self {
            id: Some(Cow::Owned(Uuid::new_v4().to_string())),
            code: error_with_context.classification.code(),
            title: error_with_context.classification.title().map(Into::into),
            status: error_with_context.classification.status().into(),
            // just output the stringified error as detail for now
            detail: Some(Cow::Owned(error_with_context.original_error.to_string())),
            ..Default::default()
        }
    }
}

mod hem {
    use crate::errors::{FhsApiErrorClassification, LegacyClassification};
    use home_energy_model_wrapper_fhs::HemError;

    impl From<&HemError> for FhsApiErrorClassification {
        fn from(hem_error: &HemError) -> Self {
            FhsApiErrorClassification::Legacy(match hem_error {
                HemError::InvalidRequest(_) => LegacyClassification::ClientError,
                HemError::NotImplemented(_) => LegacyClassification::NotImplemented,
                _ => LegacyClassification::ServerError,
            })
        }
    }
}

mod hem_database {
    use crate::errors::{FhsApiErrorClassification, LegacyClassification};
    use enum_assoc::Assoc;
    use lambda_http::http::StatusCode;
    use resolve_products::FuelType;
    use resolve_products::errors::{ResolvePcdbProductsError, SingleOrList};

    #[derive(Assoc, Debug)]
    #[func(pub fn code(&self) -> u16)]
    #[func(pub fn title(&self) -> &'static str)]
    #[func(pub fn status(&self) -> StatusCode)]
    pub enum HemDatabaseLayerErrorClassification {
        #[assoc(
            code = 600,
            title = "General error in HEM database layer",
            status = StatusCode::INTERNAL_SERVER_ERROR
        )]
        #[allow(dead_code)]
        General(String),
        #[assoc(
            code = 601,
            title = "HEM database product references found of unexpected category",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        ProductReferencesOfWrongCategory(Vec<String>),
        #[assoc(
            code = 602,
            title = "HEM database product references found of unsupported category",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        ProductReferencesOfUnsupportedCategory {
            category: String,
            product_reference: String,
        },
        #[assoc(
            code = 603,
            title = "Unknown references found to products in HEM database",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        UnknownProductReferences(SingleOrList<String>),
        #[assoc(
            code = 604,
            title = "No energy supply provided for fuel used by referenced product in HEM database",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        NoEnergySupplyForFuel(FuelType),
        #[assoc(
            code = 605,
            title = "Unrecognised sub heat network name found",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        UnknownSubHeatNetwork {
            sub_heat_network_name: String,
            heat_network_id: String,
        },
        #[assoc(
            code = 606,
            title = "Booster heat pump needed for heat network",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        BoosterHeatPumpNeededForHeatNetwork,
        #[assoc(
            code = 607,
            title = "Invalid combination of input and HEM database product data",
            status = StatusCode::UNPROCESSABLE_ENTITY
        )]
        InvalidCombination(String),
    }

    impl From<&ResolvePcdbProductsError> for FhsApiErrorClassification {
        fn from(e: &ResolvePcdbProductsError) -> Self {
            #[allow(unreachable_patterns)]
            match e {
                // N.B. commented out variants here need to be more specifically mapped
                ResolvePcdbProductsError::InvalidJson
                | ResolvePcdbProductsError::InvalidRequest(_)
                | ResolvePcdbProductsError::InvalidRequestEncounteredAfterSchemaCheck(_)
                | ResolvePcdbProductsError::InvalidProductReferenceJson(_) => {
                    FhsApiErrorClassification::Legacy(LegacyClassification::ClientError)
                }
                ResolvePcdbProductsError::InvalidCombination(message) => {
                    FhsApiErrorClassification::ResolvePcdbProducts(
                        HemDatabaseLayerErrorClassification::InvalidCombination(message.clone()),
                    )
                }
                ResolvePcdbProductsError::CouldNotExtractProductReferences(_) => {
                    FhsApiErrorClassification::Legacy(LegacyClassification::ServerError)
                }
                ResolvePcdbProductsError::ProductCategoryMismatches(references) => {
                    FhsApiErrorClassification::ResolvePcdbProducts(
                        HemDatabaseLayerErrorClassification::ProductReferencesOfWrongCategory(
                            references.clone(),
                        ),
                    )
                }
                ResolvePcdbProductsError::UnsupportedProductCategory {
                    category,
                    product_reference,
                } => FhsApiErrorClassification::ResolvePcdbProducts(
                    HemDatabaseLayerErrorClassification::ProductReferencesOfUnsupportedCategory {
                        category: category.clone(),
                        product_reference: product_reference.clone(),
                    },
                ),
                ResolvePcdbProductsError::UnknownProductReferences(references) => {
                    FhsApiErrorClassification::ResolvePcdbProducts(
                        HemDatabaseLayerErrorClassification::UnknownProductReferences(
                            references.clone(),
                        ),
                    )
                }
                ResolvePcdbProductsError::InvalidProduct(_, _)
                | ResolvePcdbProductsError::DeserializeError(_)
                | ResolvePcdbProductsError::InUseFactorsInaccessibleError
                | ResolvePcdbProductsError::InUseFactorEntryMissingError => {
                    FhsApiErrorClassification::Legacy(LegacyClassification::ServerError)
                }
                ResolvePcdbProductsError::AccessError(_) => {
                    FhsApiErrorClassification::Legacy(LegacyClassification::TransientServerError)
                }
                ResolvePcdbProductsError::NoEnergySupplyProvidedForFuelType(fuel_type) => {
                    FhsApiErrorClassification::ResolvePcdbProducts(
                        HemDatabaseLayerErrorClassification::NoEnergySupplyForFuel(*fuel_type),
                    )
                }
                ResolvePcdbProductsError::SubHeatNetworkNotFoundError(
                    sub_heat_network_name,
                    heat_network_id,
                ) => FhsApiErrorClassification::ResolvePcdbProducts(
                    HemDatabaseLayerErrorClassification::UnknownSubHeatNetwork {
                        sub_heat_network_name: sub_heat_network_name.clone(),
                        heat_network_id: heat_network_id.clone(),
                    },
                ),
                ResolvePcdbProductsError::BoosterHeatPumpNotPresentError => {
                    FhsApiErrorClassification::ResolvePcdbProducts(
                        HemDatabaseLayerErrorClassification::BoosterHeatPumpNeededForHeatNetwork,
                    )
                }
                #[cfg(test)]
                _ => FhsApiErrorClassification::Legacy(LegacyClassification::ServerError),
            }
        }
    }
}

mod json_api {
    #![allow(dead_code)]

    use json_pointer::JsonPointer;
    use lambda_http::http::StatusCode;
    use serde::{Serialize, Serializer};
    use serde_with::skip_serializing_none;
    use std::borrow::Cow;
    use url::Url;

    #[skip_serializing_none]
    #[derive(Debug, Default, Serialize)]
    pub struct ErrorObject<'a> {
        pub(super) id: Option<Cow<'a, str>>,
        pub(super) links: Option<ErrorLinks<'a>>,
        #[serde(serialize_with = "status_to_string")]
        pub(super) status: Option<StatusCode>,
        // N.B. `code` should be serialised as a string, but we are constraining it to a number here in the struct
        #[serde(serialize_with = "code_to_string")]
        pub(super) code: Option<u16>,
        pub(super) title: Option<Cow<'a, str>>,
        pub(super) detail: Option<Cow<'a, str>>,
        pub(super) source: Option<ErrorSource<'a>>,
    }

    fn status_to_string<S: Serializer>(
        status_code: &Option<StatusCode>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match status_code {
            None => serializer.serialize_none(),
            Some(status_code) => serializer.serialize_str(status_code.as_str()),
        }
    }

    fn code_to_string<S: Serializer>(
        pointer: &Option<u16>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match pointer {
            None => serializer.serialize_none(),
            Some(pointer) => serializer.serialize_str(pointer.to_string().as_str()),
        }
    }

    #[skip_serializing_none]
    #[derive(Debug, Serialize)]
    pub struct ErrorLinks<'a> {
        about: Option<Link<'a>>,
        #[serde(rename = "type")]
        type_: Option<Link<'a>>,
    }

    #[skip_serializing_none]
    #[derive(Debug, Serialize)]
    pub struct ErrorSource<'a> {
        /// A JSON Pointer [RFC6901] to the associated entity in the request document [e.g. "/data" for a primary data object, or "/data/attributes/title" for a specific attribute].
        #[serde(serialize_with = "pointer_to_string")]
        pointer: Option<JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>>,
        /// A string indicating which query parameter caused the error.
        parameter: Option<Cow<'a, str>>,
        /// A string indicating the name of a single request header which caused the error.
        header: Option<Cow<'a, str>>,
    }

    fn pointer_to_string<'a, S: Serializer>(
        pointer: &Option<JsonPointer<Cow<'a, str>, Vec<Cow<'a, str>>>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match pointer {
            None => serializer.serialize_none(),
            Some(pointer) => serializer.serialize_str(pointer.to_string().as_str()),
        }
    }

    #[skip_serializing_none]
    #[derive(Debug, Serialize)]
    pub struct LinkObject<'a> {
        href: LinkUrl<'a>,
        rel: Option<Cow<'a, str>>,
        title: Option<Cow<'a, str>>,
        #[serde(rename = "type")]
        type_: Option<Cow<'a, str>>,
        hreflang: Option<Cow<'a, str>>,
        describedby: Option<Box<Link<'a>>>,
    }

    /// A link **MUST** be represented as either: a string containing the link's URL or a link object.
    #[derive(Debug, Serialize)]
    #[serde(untagged)]
    pub enum Link<'a> {
        Url(LinkUrl<'a>),
        Object(LinkObject<'a>),
    }

    #[derive(Debug, Serialize)]
    pub struct LinkUrl<'a>(Cow<'a, Url>);
}
