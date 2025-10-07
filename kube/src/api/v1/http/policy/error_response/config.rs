use super::{ErrorResponsePolicy, ErrorResponsePolicyFormat, ProblemDetailErrorResponseFormat};
use http::Uri;
use http::uri::InvalidUri;
use thiserror::Error;
use vg_config::http::policy::error_response::{
    ErrorResponsePolicy as ErrorResponsePolicyConfig, Format, ProblemDetailFormat,
};

impl TryFrom<&ErrorResponsePolicy> for ErrorResponsePolicyConfig {
    type Error = ErrorResponsePolicyConversionError;

    fn try_from(value: &ErrorResponsePolicy) -> Result<Self, Self::Error> {
        let builder = Self::builder();

        let builder = match (&value.format, value.problem_detail.as_ref()) {
            (ErrorResponsePolicyFormat::Empty, None) => builder.format(Format::Empty),
            (ErrorResponsePolicyFormat::Html, None) => builder.format(Format::Html),
            (ErrorResponsePolicyFormat::ProblemDetail, Some(problem_detail)) => {
                let format = ProblemDetailFormat::try_from(problem_detail)?;
                builder.format(format)
            }
            (ErrorResponsePolicyFormat::ProblemDetail, None) => {
                return Err(ErrorResponsePolicyConversionError::MissingProblemDetail);
            }
            _ => {
                return Err(ErrorResponsePolicyConversionError::InvalidConfiguration);
            }
        };

        let policy = builder.build();

        Ok(policy)
    }
}

#[derive(Debug, Error)]
pub enum ErrorResponsePolicyConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("`problem_detail` is required for 'ProblemDetail' kind")]
    MissingProblemDetail,
    #[error("Problem detail configuration error: {0}")]
    ProblemDetail(
        #[from]
        #[source]
        ProblemDetailErrorResponseGeneratorConversionError,
    ),
}

#[derive(Debug, Error)]
pub enum ProblemDetailErrorResponseGeneratorConversionError {
    #[error("Invalid problem detail authority URI: {0}")]
    Authority(
        #[from]
        #[source]
        InvalidUri,
    ),
}

impl TryFrom<&ProblemDetailErrorResponseFormat> for ProblemDetailFormat {
    type Error = ProblemDetailErrorResponseGeneratorConversionError;

    fn try_from(value: &ProblemDetailErrorResponseFormat) -> Result<Self, Self::Error> {
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
