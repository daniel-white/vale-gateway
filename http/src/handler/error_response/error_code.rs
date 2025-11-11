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
    BackendUnavailable,
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
            Self::BackendUnavailable => "Backend unavailable".into(),
            Self::InvalidConfiguration => "Invalid configuration".into(),
            Self::StatusCode(status) => status
                .canonical_reason()
                .unwrap_or("Unknown status code")
                .into(),
        }
    }
}

impl From<ErrorResponseCode> for StatusCode {
    fn from(code: ErrorResponseCode) -> Self {
        match code {
            ErrorResponseCode::NoRoute => Self::NOT_FOUND,
            ErrorResponseCode::AccessDenied => Self::FORBIDDEN,
            ErrorResponseCode::MissingConfiguration => Self::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::BackendUnavailable => Self::SERVICE_UNAVAILABLE,
            ErrorResponseCode::InvalidConfiguration => Self::INTERNAL_SERVER_ERROR,
            ErrorResponseCode::StatusCode(status) => status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case(ErrorResponseCode::NoRoute, StatusCode::NOT_FOUND)]
    #[case(ErrorResponseCode::AccessDenied, StatusCode::FORBIDDEN)]
    #[case(
        ErrorResponseCode::MissingConfiguration,
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    #[case(ErrorResponseCode::BackendUnavailable, StatusCode::SERVICE_UNAVAILABLE)]
    #[case(
        ErrorResponseCode::InvalidConfiguration,
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    fn test_error_code_mapping_to_http_status(
        #[case] error_code: ErrorResponseCode,
        #[case] expected_status: StatusCode,
    ) {
        assert_eq!(StatusCode::from(error_code), expected_status);
    }

    #[rstest]
    #[case(ErrorResponseCode::NoRoute, "No matching route found")]
    #[case(ErrorResponseCode::AccessDenied, "Access denied")]
    #[case(ErrorResponseCode::MissingConfiguration, "Missing configuration")]
    #[case(ErrorResponseCode::BackendUnavailable, "Backend unavailable")]
    #[case(ErrorResponseCode::InvalidConfiguration, "Invalid configuration")]
    fn test_error_code_descriptions(
        #[case] error_code: ErrorResponseCode,
        #[case] expected_message: &str,
    ) {
        assert_eq!(error_code.message(), expected_message);
    }

    #[rstest]
    #[case(StatusCode::BAD_REQUEST, StatusCode::BAD_REQUEST)]
    #[case(StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED)]
    #[case(StatusCode::BAD_GATEWAY, StatusCode::BAD_GATEWAY)]
    fn test_status_code_variant_mapping(
        #[case] input_status: StatusCode,
        #[case] expected_status: StatusCode,
    ) {
        let error_code = ErrorResponseCode::StatusCode(input_status);
        assert_eq!(StatusCode::from(error_code), expected_status);
    }

    #[rstest]
    #[case(StatusCode::BAD_REQUEST, "Bad Request")]
    #[case(StatusCode::UNAUTHORIZED, "Unauthorized")]
    #[case(StatusCode::IM_A_TEAPOT, "I'm a teapot")]
    fn test_status_code_variant_descriptions(
        #[case] input_status: StatusCode,
        #[case] expected_message: &str,
    ) {
        let error_code = ErrorResponseCode::StatusCode(input_status);
        assert_eq!(error_code.message(), expected_message);
    }

    #[test]
    fn test_status_code_variant_descriptions_unknown() {
        // Test with a status code that doesn't have a canonical reason
        let custom_status = StatusCode::from_u16(299).unwrap();
        let custom_code = ErrorResponseCode::StatusCode(custom_status);
        assert_eq!(custom_code.message(), "Unknown status code");
    }

    #[rstest]
    #[case(ErrorResponseCode::NoRoute, "NO_ROUTE")]
    #[case(ErrorResponseCode::AccessDenied, "ACCESS_DENIED")]
    #[case(ErrorResponseCode::MissingConfiguration, "MISSING_CONFIGURATION")]
    #[case(ErrorResponseCode::BackendUnavailable, "BACKEND_UNAVAILABLE")]
    #[case(ErrorResponseCode::InvalidConfiguration, "INVALID_CONFIGURATION")]
    fn test_to_str_error_variants(
        #[case] error_code: ErrorResponseCode,
        #[case] expected_str: &str,
    ) {
        assert_eq!(error_code.to_str(), expected_str);
    }

    #[rstest]
    #[case(StatusCode::BAD_REQUEST, "BAD_REQUEST")]
    #[case(StatusCode::UNAUTHORIZED, "UNAUTHORIZED")]
    #[case(StatusCode::IM_A_TEAPOT, "IM_A_TEAPOT")]
    #[case(StatusCode::NOT_FOUND, "NOT_FOUND")]
    #[case(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR")]
    fn test_to_str_status_code_variants(
        #[case] status_code: StatusCode,
        #[case] expected_str: &str,
    ) {
        let error_code = ErrorResponseCode::StatusCode(status_code);
        assert_eq!(error_code.to_str(), expected_str);
    }

    #[test]
    fn test_to_str_unknown_status() {
        // Test with a status code that doesn't have a canonical reason
        let custom_status = StatusCode::from_u16(299).unwrap();
        let custom_code = ErrorResponseCode::StatusCode(custom_status);
        assert_eq!(custom_code.to_str(), "UNKNOWN_ERROR");
    }

    #[rstest]
    #[case(ErrorResponseCode::NoRoute, "NO_ROUTE")]
    #[case(ErrorResponseCode::AccessDenied, "ACCESS_DENIED")]
    #[case(ErrorResponseCode::MissingConfiguration, "MISSING_CONFIGURATION")]
    #[case(ErrorResponseCode::BackendUnavailable, "BACKEND_UNAVAILABLE")]
    #[case(ErrorResponseCode::InvalidConfiguration, "INVALID_CONFIGURATION")]
    fn test_into_static_str(#[case] error_code: ErrorResponseCode, #[case] expected_str: &str) {
        let result: &'static str = error_code.into();
        assert_eq!(result, expected_str);
    }

    #[test]
    fn test_status_code_descriptions() {
        let im_a_teapot_msg = ErrorResponseCode::StatusCode(StatusCode::IM_A_TEAPOT).message();

        assert_eq!(im_a_teapot_msg, "I'm a teapot");
    }
}
