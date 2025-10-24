pub mod error_codes;
mod generators;

use crate::policy::error_response::error_codes::ErrorResponseCode;
use crate::policy::error_response::generators::{
    ErrorResponseGenerator, ErrorResponseGeneratorConversionError,
};
use bytes::Bytes;
use http::Response;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::policy::error_response::ErrorResponsePolicy;

#[derive(Debug, TypedBuilder)]
pub struct ErrorResponsePolicyHandler {
    #[builder(setter(into))]
    generator: ErrorResponseGenerator,
}

impl ErrorResponsePolicyHandler {
    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        self.generator.generate_response(code)
    }
}

#[derive(Debug, Error)]
pub enum ErrorResponsePolicyHandlerConversionError {
    #[error(transparent)]
    Generator(#[from] ErrorResponseGeneratorConversionError),
}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&ErrorResponsePolicy> for ErrorResponsePolicyHandler {
    type Error = ErrorResponsePolicyHandlerConversionError;

    fn try_from(value: &ErrorResponsePolicy) -> Result<Self, Self::Error> {
        let generator = ErrorResponseGenerator::try_from(value.format())?;

        let handler = Self::builder().generator(generator).build();

        Ok(handler)
    }
}
