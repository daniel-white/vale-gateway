use crate::api::v1::http::filters::error_response::{
    ErrorResponseFilterKind, ErrorResponseFilterSpec, ProblemDetailErrorResponse,
};
use http::Uri;
use http::uri::InvalidUri;
use thiserror::Error;
use vg_http_config::filters::error_response::{
    ErrorResponseFilter, ErrorResponseGenerator, ProblemDetailErrorResponseGenerator,
};

#[derive(Debug, Error)]
pub enum ErrorResponseFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("`problem_detail` is required for 'ProblemDetail' kind")]
    MissingProblemDetail,
    #[error("Problem detail configuration error: {0}")]
    ProblemDetail(#[from] ProblemDetailErrorResponseGeneratorConversionError),
}

impl TryFrom<&ErrorResponseFilterSpec> for ErrorResponseFilter {
    type Error = ErrorResponseFilterConversionError;

    fn try_from(value: &ErrorResponseFilterSpec) -> Result<Self, Self::Error> {
        let builder = Self::builder();

        let builder = match (&value.kind, value.problem_detail.as_ref()) {
            (ErrorResponseFilterKind::Empty, None) => {
                builder.generator(ErrorResponseGenerator::Empty)
            }
            (ErrorResponseFilterKind::Html, None) => {
                builder.generator(ErrorResponseGenerator::Html)
            }
            (ErrorResponseFilterKind::ProblemDetail, Some(problem_detail)) => {
                let generator = ProblemDetailErrorResponseGenerator::try_from(problem_detail)?;
                builder.generator(generator)
            }
            (ErrorResponseFilterKind::ProblemDetail, None) => {
                return Err(ErrorResponseFilterConversionError::MissingProblemDetail);
            }
            _ => {
                return Err(ErrorResponseFilterConversionError::InvalidConfiguration);
            }
        };

        let filter = builder.build();

        Ok(filter)
    }
}

#[derive(Debug, Error)]
pub enum ProblemDetailErrorResponseGeneratorConversionError {
    #[error("Invalid problem detail authority URI: {0}")]
    Authority(#[from] InvalidUri),
}

impl TryFrom<&ProblemDetailErrorResponse> for ProblemDetailErrorResponseGenerator {
    type Error = ProblemDetailErrorResponseGeneratorConversionError;

    fn try_from(value: &ProblemDetailErrorResponse) -> Result<Self, Self::Error> {
        let authority = match &value.authority {
            Some(authority) => {
                let uri: Uri = authority.parse()?;
                Some(uri)
            }
            None => None,
        };

        let generator = Self::builder().authority(authority).build();

        Ok(generator)
    }
}
