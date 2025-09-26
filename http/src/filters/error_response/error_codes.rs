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

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::*;

    #[tokio::test]
    async fn test_error_code_mapping_to_http_status() {
        // Test mapping custom error codes to HTTP status codes
        assert_eq!(
            StatusCode::from(ErrorResponseCode::NoRoute),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            StatusCode::from(ErrorResponseCode::AccessDenied),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            StatusCode::from(ErrorResponseCode::MissingConfiguration),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            StatusCode::from(ErrorResponseCode::UpstreamUnavailable),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            StatusCode::from(ErrorResponseCode::InvalidConfiguration),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_error_code_descriptions() {
        // Test the message descriptions for error codes
        let no_route_msg: Cow<'static, str> = ErrorResponseCode::NoRoute.into();
        let access_denied_msg: Cow<'static, str> = ErrorResponseCode::AccessDenied.into();
        let missing_config_msg: Cow<'static, str> = ErrorResponseCode::MissingConfiguration.into();
        let upstream_unavailable_msg: Cow<'static, str> =
            ErrorResponseCode::UpstreamUnavailable.into();
        let invalid_config_msg: Cow<'static, str> = ErrorResponseCode::InvalidConfiguration.into();

        assert_eq!(no_route_msg, "No matching route found");
        assert_eq!(access_denied_msg, "Access denied");
        assert_eq!(missing_config_msg, "Missing configuration");
        assert_eq!(upstream_unavailable_msg, "Upstream unavailable");
        assert_eq!(invalid_config_msg, "Invalid configuration");
    }
}
