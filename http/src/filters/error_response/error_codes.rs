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
    StatusCode(StatusCode),
}

impl From<ErrorResponseCode> for StatusCode {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => Self::NOT_FOUND,
            ErrorResponseCode::AccessDenied => Self::FORBIDDEN,
            ErrorResponseCode::MissingConfiguration => Self::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::UpstreamUnavailable => Self::SERVICE_UNAVAILABLE,
            ErrorResponseCode::InvalidConfiguration => Self::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::StatusCode(status) => status,
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
            ErrorResponseCode::StatusCode(status) => {
                status.canonical_reason().unwrap_or("Unknown error").into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_mapping_to_http_status() {
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

    #[test]
    fn test_error_code_descriptions() {
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

    #[test]
    fn test_status_code_variant_mapping() {
        // Test that StatusCode variant passes through the status code unchanged
        let custom_status = StatusCode::BAD_REQUEST;
        let error_code = ErrorResponseCode::StatusCode(custom_status);
        assert_eq!(StatusCode::from(error_code), StatusCode::BAD_REQUEST);

        let another_status = StatusCode::UNAUTHORIZED;
        let another_error_code = ErrorResponseCode::StatusCode(another_status);
        assert_eq!(
            StatusCode::from(another_error_code),
            StatusCode::UNAUTHORIZED
        );

        let server_error = StatusCode::BAD_GATEWAY;
        let server_error_code = ErrorResponseCode::StatusCode(server_error);
        assert_eq!(StatusCode::from(server_error_code), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_status_code_variant_descriptions() {
        // Test message descriptions for StatusCode variant
        let bad_request_code = ErrorResponseCode::StatusCode(StatusCode::BAD_REQUEST);
        let bad_request_msg: Cow<'static, str> = bad_request_code.into();
        assert_eq!(bad_request_msg, "Bad Request");

        let unauthorized_code = ErrorResponseCode::StatusCode(StatusCode::UNAUTHORIZED);
        let unauthorized_msg: Cow<'static, str> = unauthorized_code.into();
        assert_eq!(unauthorized_msg, "Unauthorized");

        let teapot_code = ErrorResponseCode::StatusCode(StatusCode::IM_A_TEAPOT);
        let teapot_msg: Cow<'static, str> = teapot_code.into();
        assert_eq!(teapot_msg, "I'm a teapot");

        // Test with a status code that doesn't have a canonical reason
        let custom_status = StatusCode::from_u16(299).unwrap();
        let custom_code = ErrorResponseCode::StatusCode(custom_status);
        let custom_msg: Cow<'static, str> = custom_code.into();
        assert_eq!(custom_msg, "Unknown error");
    }
}
