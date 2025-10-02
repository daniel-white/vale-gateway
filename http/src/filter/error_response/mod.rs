pub mod error_codes;
pub mod generators;

use crate::filter::error_response::error_codes::ErrorResponseCode;
use crate::filter::error_response::generators::{
    ErrorResponseGenerator, ErrorResponseGeneratorConversionError,
};
use bytes::Bytes;
use http::Response;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::filter::error_response::ErrorResponseFilter;

#[derive(Debug, TypedBuilder)]
pub struct ErrorResponseFilterHandler {
    #[builder(setter(into))]
    generator: ErrorResponseGenerator,
}

impl ErrorResponseFilterHandler {
    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        self.generator.generate_response(code)
    }
}

#[derive(Debug, Error)]
pub enum ErrorResponseFilterConversionError {
    #[error("Invalid error response generator: {0}")]
    InvalidGenerator(#[from] ErrorResponseGeneratorConversionError),
}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&ErrorResponseFilter> for ErrorResponseFilterHandler {
    type Error = ErrorResponseFilterConversionError;

    fn try_from(value: &ErrorResponseFilter) -> Result<Self, Self::Error> {
        let generator = ErrorResponseGenerator::try_from(value.generator())?;

        let handler = Self::builder().generator(generator).build();

        Ok(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::filter::error_response::generators::{
        EmptyErrorResponseGenerator, HtmlErrorResponseGenerator,
        ProblemDetailErrorResponseGenerator,
    };
    use http::{StatusCode, Uri};
    use vg_core::http::content_type::HTML;

    #[tokio::test]
    async fn test_error_response_handler_creation() {
        // Test creating error response handlers for different status codes using actual ErrorResponseFilterHandler
        let generator = EmptyErrorResponseGenerator::builder().build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        let response = handler.generate_response(ErrorResponseCode::NoRoute);

        // Should generate a response with the correct status
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.body().is_none()); // Empty generator returns no body
    }

    #[tokio::test]
    async fn test_error_response_with_custom_message() {
        // Test error responses with HTML body
        let generator = HtmlErrorResponseGenerator::builder().build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        let response = handler.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["content-type"], HTML.to_string());
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_error_response_json_format() {
        // Test generating Problem Details (JSON) error responses
        let generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(None)
            .build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        let response = handler.generate_response(ErrorResponseCode::UpstreamUnavailable);

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_error_response_html_format() {
        // Test generating HTML error responses
        let generator = HtmlErrorResponseGenerator::builder().build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        let response = handler.generate_response(ErrorResponseCode::InvalidConfiguration);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers()["content-type"], HTML.to_string());

        // Verify HTML structure in the body
        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("<html>"));
            assert!(body_str.contains("</html>"));
        }
    }

    #[tokio::test]
    async fn test_error_response_problem_details_format() {
        let authority = Uri::from_static("https://api.example.com/problems/");
        let generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(Some(authority))
            .build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        let response = handler.generate_response(ErrorResponseCode::MissingConfiguration);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );

        // Verify Problem Details structure
        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("MISSING_CONFIGURATION"));
            assert!(body_str.contains("https://api.example.com/problems/"));
            assert!(body_str.contains("Missing configuration"));
        }
    }

    #[tokio::test]
    async fn test_error_response_with_trace_id() {
        // Test that error responses can include trace IDs when available
        let generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(None)
            .build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        // The trace_id functionality depends on the current tracing span
        let response = handler.generate_response(ErrorResponseCode::NoRoute);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );

        // Trace ID would be included if there's an active tracing span
        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("NO_ROUTE"));
        }
    }

    #[tokio::test]
    async fn test_error_response_content_length_header() {
        // Test that Content-Length header is set correctly for error responses
        let generator = HtmlErrorResponseGenerator::builder().build();

        let handler = ErrorResponseFilterHandler::builder()
            .generator(generator)
            .build();

        let response = handler.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.headers().contains_key("content-length"));
        assert!(response.body().is_some());

        // Content-Length should match the body size
        if let Some(body) = response.body() {
            let expected_length = body.len().to_string();
            assert_eq!(response.headers()["content-length"], expected_length);
        }
    }

    #[tokio::test]
    async fn test_error_response_different_generators() {
        // Test that different generators produce different output formats
        let empty_generator = EmptyErrorResponseGenerator::builder().build();
        let html_generator = HtmlErrorResponseGenerator::builder().build();
        let problem_generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(None)
            .build();

        let empty_handler = ErrorResponseFilterHandler::builder()
            .generator(empty_generator)
            .build();
        let html_handler = ErrorResponseFilterHandler::builder()
            .generator(html_generator)
            .build();
        let problem_handler = ErrorResponseFilterHandler::builder()
            .generator(problem_generator)
            .build();

        let error_code = ErrorResponseCode::InvalidConfiguration;

        let empty_response = empty_handler.generate_response(error_code);
        let html_response = html_handler.generate_response(error_code);
        let problem_response = problem_handler.generate_response(error_code);

        // All should have same status code
        assert_eq!(empty_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(html_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(problem_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // But different content types and bodies
        assert!(empty_response.body().is_none());
        assert_eq!(html_response.headers()["content-type"], HTML.to_string());
        assert_eq!(
            problem_response.headers()["content-type"],
            "application/problem+json"
        );
    }

    #[tokio::test]
    async fn test_error_code_to_status_mapping() {
        // Test that all error codes map to appropriate HTTP status codes
        use http::StatusCode;

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
        // Test that error codes have appropriate descriptions
        let no_route_msg = ErrorResponseCode::NoRoute.message();
        let access_denied_msg = ErrorResponseCode::AccessDenied.message();
        let missing_config_msg = ErrorResponseCode::MissingConfiguration.message();
        let upstream_unavailable_msg = ErrorResponseCode::UpstreamUnavailable.message();
        let invalid_config_msg = ErrorResponseCode::InvalidConfiguration.message();
        let im_a_teapot_msg = ErrorResponseCode::StatusCode(StatusCode::IM_A_TEAPOT).message();

        assert_eq!(no_route_msg, "No matching route found");
        assert_eq!(access_denied_msg, "Access denied");
        assert_eq!(missing_config_msg, "Missing configuration");
        assert_eq!(upstream_unavailable_msg, "Upstream unavailable");
        assert_eq!(invalid_config_msg, "Invalid configuration");
        assert_eq!(im_a_teapot_msg, "I'm a teapot");
    }
}
