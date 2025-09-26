use heck::ToShoutySnakeCase;
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

// Custom implementation to handle StatusCode canonical reasons
impl ErrorResponseCode {
    pub fn to_str(&self) -> Cow<'static, str> {
        match self {
            Self::StatusCode(status) => status
                .canonical_reason()
                .unwrap_or("Unknown Error")
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
                .to_shouty_snake_case()
                .into(),
            _ => {
                let str: &'static str = self.into();
                str.into()
            }
        }
    }

    pub fn message(&self) -> Cow<'static, str> {
        match self {
            Self::NoRoute => "No matching route found".into(),
            Self::AccessDenied => "Access denied".into(),
            Self::MissingConfiguration => "Missing configuration".into(),
            Self::UpstreamUnavailable => "Upstream unavailable".into(),
            Self::InvalidConfiguration => "Invalid configuration".into(),
            Self::StatusCode(status) => status.canonical_reason().unwrap_or("Unknown error").into(),
        }
    }
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
        // Test the message descriptions for error codes using the new .message() method
        assert_eq!(
            ErrorResponseCode::NoRoute.message(),
            "No matching route found"
        );
        assert_eq!(ErrorResponseCode::AccessDenied.message(), "Access denied");
        assert_eq!(
            ErrorResponseCode::MissingConfiguration.message(),
            "Missing configuration"
        );
        assert_eq!(
            ErrorResponseCode::UpstreamUnavailable.message(),
            "Upstream unavailable"
        );
        assert_eq!(
            ErrorResponseCode::InvalidConfiguration.message(),
            "Invalid configuration"
        );
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
        // Test message descriptions for StatusCode variant using the new .message() method
        let bad_request_code = ErrorResponseCode::StatusCode(StatusCode::BAD_REQUEST);
        assert_eq!(bad_request_code.message(), "Bad Request");

        let unauthorized_code = ErrorResponseCode::StatusCode(StatusCode::UNAUTHORIZED);
        assert_eq!(unauthorized_code.message(), "Unauthorized");

        let teapot_code = ErrorResponseCode::StatusCode(StatusCode::IM_A_TEAPOT);
        assert_eq!(teapot_code.message(), "I'm a teapot");

        // Test with a status code that doesn't have a canonical reason
        let custom_status = StatusCode::from_u16(299).unwrap();
        let custom_code = ErrorResponseCode::StatusCode(custom_status);
        assert_eq!(custom_code.message(), "Unknown error");
    }

    #[test]
    fn test_to_str() {
        // Test our custom to_str conversion for all variants
        assert_eq!(ErrorResponseCode::NoRoute.to_str(), "NO_ROUTE");
        assert_eq!(ErrorResponseCode::AccessDenied.to_str(), "ACCESS_DENIED");
        assert_eq!(
            ErrorResponseCode::MissingConfiguration.to_str(),
            "MISSING_CONFIGURATION"
        );
        assert_eq!(
            ErrorResponseCode::UpstreamUnavailable.to_str(),
            "UPSTREAM_UNAVAILABLE"
        );
        assert_eq!(
            ErrorResponseCode::InvalidConfiguration.to_str(),
            "INVALID_CONFIGURATION"
        );

        // Test StatusCode variants with canonical reasons
        let bad_request_code = ErrorResponseCode::StatusCode(StatusCode::BAD_REQUEST);
        assert_eq!(bad_request_code.to_str(), "BAD_REQUEST");

        let unauthorized_code = ErrorResponseCode::StatusCode(StatusCode::UNAUTHORIZED);
        assert_eq!(unauthorized_code.to_str(), "UNAUTHORIZED");

        // The famous teapot test - strips the apostrophe!
        let teapot_code = ErrorResponseCode::StatusCode(StatusCode::IM_A_TEAPOT);
        assert_eq!(teapot_code.to_str(), "IM_A_TEAPOT");

        // Test with a status code that doesn't have a canonical reason
        let custom_status = StatusCode::from_u16(299).unwrap();
        let custom_code = ErrorResponseCode::StatusCode(custom_status);
        assert_eq!(custom_code.to_str(), "UNKNOWN_ERROR");

        // Test more complex status codes
        let not_found_code = ErrorResponseCode::StatusCode(StatusCode::NOT_FOUND);
        assert_eq!(not_found_code.to_str(), "NOT_FOUND");

        let internal_server_error_code =
            ErrorResponseCode::StatusCode(StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(internal_server_error_code.to_str(), "INTERNAL_SERVER_ERROR");
    }

    #[test]
    fn test_into_static_str() {
        // Test strum's IntoStaticStr conversion for all variants
        let no_route_str: &'static str = ErrorResponseCode::NoRoute.into();
        assert_eq!(no_route_str, "NO_ROUTE");

        let access_denied_str: &'static str = ErrorResponseCode::AccessDenied.into();
        assert_eq!(access_denied_str, "ACCESS_DENIED");

        let missing_config_str: &'static str = ErrorResponseCode::MissingConfiguration.into();
        assert_eq!(missing_config_str, "MISSING_CONFIGURATION");

        let upstream_unavailable_str: &'static str = ErrorResponseCode::UpstreamUnavailable.into();
        assert_eq!(upstream_unavailable_str, "UPSTREAM_UNAVAILABLE");

        let invalid_config_str: &'static str = ErrorResponseCode::InvalidConfiguration.into();
        assert_eq!(invalid_config_str, "INVALID_CONFIGURATION");

        // Test StatusCode variant - strum will just return "STATUS_CODE"
        let status_code_str: &'static str =
            ErrorResponseCode::StatusCode(StatusCode::BAD_REQUEST).into();
        assert_eq!(status_code_str, "STATUS_CODE");

        // All StatusCode variants return the same string with strum
        let another_status_str: &'static str =
            ErrorResponseCode::StatusCode(StatusCode::IM_A_TEAPOT).into();
        assert_eq!(another_status_str, "STATUS_CODE");
    }
}
