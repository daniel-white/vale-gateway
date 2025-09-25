use http::StatusCode;
use std::borrow::Cow;
use strum::IntoStaticStr;

#[derive(Debug, Clone, Copy, IntoStaticStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorResponseCode {
    NoRoute,
    AccessDenied,
    MissingConfiguration,
    UpstreamUnavailable,
    InvalidConfiguration,
}

impl From<ErrorResponseCode> for StatusCode {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => Self::NOT_FOUND,
            ErrorResponseCode::AccessDenied => Self::FORBIDDEN,
            ErrorResponseCode::MissingConfiguration => Self::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::UpstreamUnavailable => Self::SERVICE_UNAVAILABLE,
            ErrorResponseCode::InvalidConfiguration => Self::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ErrorResponseCode> for Cow<'static, str> {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => "No matching route found".into(),
            ErrorResponseCode::AccessDenied => "Access denied".into(),
            ErrorResponseCode::MissingConfiguration => "Missing configuration".into(),
            ErrorResponseCode::UpstreamUnavailable => "Upstream unavailable".into(),
            ErrorResponseCode::InvalidConfiguration => "Invalid configuration".into(),
        }
    }
}
