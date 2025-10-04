use gateway_api::httproutes::HTTPBackendReference;
use thiserror::Error;
use typed_builder::TypedBuilder;
use crate::api::v1::http::route::filter::config::{RuleBackendFilterConversionError};

#[derive(Debug, TypedBuilder)]
pub struct HTTPBackendReferenceWrapper<'a> {
    namespace: &'a str,
    backend_ref: &'a HTTPBackendReference,
}

#[derive(Debug, Error)]
pub enum RuleBackendConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid port")]
    Port,
    #[error("Invalid filter at index {0}: {1}")]
    Filter(usize, RuleBackendFilterConversionError),
}
