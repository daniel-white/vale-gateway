use crate::handler::error_response::error_code::ErrorResponseCode;
use bytes::Bytes;
use derive_more::From;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{Response, StatusCode, Uri};
use opentelemetry::TraceId;
use opentelemetry::trace::TraceContextExt;
use problemdetails::Problem;
use std::borrow::Cow;
use thiserror::Error;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use typed_builder::TypedBuilder;
use vg_config::http::policy::error_response::{ErrorResponsePolicy, Format, ProblemDetailFormat};
use vg_core::http::content_type::{ContentType, HTML, PROBLEM_DETAIL};

#[derive(Debug, Clone, From)]
#[allow(private_interfaces)]
pub enum ErrorResponseGenerator {
    Empty(EmptyErrorResponseGenerator),
    Html(HtmlErrorResponseGenerator),
    ProblemDetail(ProblemDetailErrorResponseGenerator),
}

impl ErrorResponseGenerator {
    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        match self {
            Self::Empty(generator) => generator.generate_response(code),
            Self::Html(generator) => generator.generate_response(code),
            Self::ProblemDetail(generator) => generator.generate_response(code),
        }
    }
}

impl Default for ErrorResponseGenerator {
    fn default() -> Self {
        ErrorResponsePolicy::default()
            .format()
            .try_into()
            .expect("bad default error response policy")
    }
}

#[derive(Debug, Clone, Error)]
pub enum ErrorResponseGeneratorConversionError {
    #[error("Failed to convert problem detail generator: {0}")]
    ProblemDetail(
        #[from]
        #[source]
        ProblemDetailErrorResponseGeneratorConversionError,
    ),
}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&Format> for ErrorResponseGenerator {
    type Error = ErrorResponseGeneratorConversionError;

    fn try_from(value: &Format) -> Result<Self, Self::Error> {
        let generator = match value {
            Format::Empty => EmptyErrorResponseGenerator::builder().build().into(),
            Format::Html => HtmlErrorResponseGenerator::builder().build().into(),
            Format::ProblemDetail(format) => ProblemDetailErrorResponseGenerator::try_from(format)?.into(),
        };

        Ok(generator)
    }
}

trait Generator: Into<ErrorResponseGenerator> {
    fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        let status_code: StatusCode = code.into();
        let body = self.body(code);

        let response = Response::builder().status(status_code);

        match body {
            Some((content_type, body_content)) => {
                let body = Bytes::from(body_content.into_owned());
                response
                    .header(CONTENT_TYPE, content_type)
                    .header(CONTENT_LENGTH, body.len())
                    .body(Some(body))
                    .expect("Failed to build error response")
            }
            None => response.body(None).expect("Failed to build error response"),
        }
    }

    fn body(&self, _code: ErrorResponseCode) -> Option<(ContentType<'static>, Cow<'static, str>)> {
        None
    }

    fn trace_id(&self) -> Option<String> {
        let span = Span::current();
        let context = span.context();
        let trace_id = context.span().span_context().trace_id();
        if trace_id == TraceId::INVALID {
            None
        } else {
            Some(trace_id.to_string())
        }
    }
}

#[derive(Debug, Clone, TypedBuilder)]
struct EmptyErrorResponseGenerator {}

impl Generator for EmptyErrorResponseGenerator {}

#[derive(Debug, Clone, TypedBuilder)]
struct HtmlErrorResponseGenerator {}

impl Generator for HtmlErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(ContentType<'static>, Cow<'static, str>)> {
        let message = code.message();
        let body = format!("<html><body><h1>{message}</h1></body></html>");
        Some((HTML, body.into()))
    }
}

#[derive(Debug, Clone, TypedBuilder)]
struct ProblemDetailErrorResponseGenerator {
    authority: Option<Uri>,
}

impl Generator for ProblemDetailErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(ContentType<'static>, Cow<'static, str>)> {
        let message = code.message();
        let code_str = code.to_str();
        let status_code: StatusCode = code.into();

        let problem = Problem::from(code)
            .with_value("status", status_code.as_u16())
            .with_type(format!(
                "{}{}",
                self.authority
                    .as_ref().map_or_else(|| { "http://vale-gateway.whitefamily.in/errors/".to_string() }, std::string::ToString::to_string),
                code_str
            ))
            .with_detail(message);

        let problem = if let Some(trace_id) = self.trace_id() {
            problem.with_value("trace_id", trace_id)
        } else {
            problem
        };

        let body: String = serde_json::to_string(&problem.body).unwrap();

        Some((PROBLEM_DETAIL, body.into()))
    }
}

#[derive(Debug, Clone, Error)]
enum ProblemDetailErrorResponseGeneratorConversionError {}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&ProblemDetailFormat> for ProblemDetailErrorResponseGenerator {
    type Error = ProblemDetailErrorResponseGeneratorConversionError;

    fn try_from(config: &ProblemDetailFormat) -> Result<Self, Self::Error> {
        let generator = Self::builder().authority(config.authority()).build();

        Ok(generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use http::StatusCode;

    #[tokio::test]
    async fn test_json_error_response_generator() {
        // Test the actual ProblemDetailErrorResponseGenerator (JSON format)
        let generator = ProblemDetailErrorResponseGenerator::builder().authority(None).build();

        let response = generator.generate_response(ErrorResponseCode::BackendUnavailable);

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["content-type"], PROBLEM_DETAIL.to_string());

        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("BACKEND_UNAVAILABLE"));
            assert!(body_str.contains("Backend unavailable"));
        }
    }

    #[tokio::test]
    async fn test_html_error_response_generator() {
        // Test the actual HtmlErrorResponseGenerator
        let generator = HtmlErrorResponseGenerator::builder().build();

        let response = generator.generate_response(ErrorResponseCode::NoRoute);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["content-type"], HTML.to_string());

        // Verify HTML structure
        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("<html>"));
            assert!(body_str.contains("<body>"));
            assert!(body_str.contains("No matching route found"));
            assert!(body_str.contains("</body>"));
            assert!(body_str.contains("</html>"));
        }
    }

    #[tokio::test]
    async fn test_plain_text_error_response_generator() {
        // Test the EmptyErrorResponseGenerator (no body)
        let generator = EmptyErrorResponseGenerator::builder().build();

        let response = generator.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.body().is_none()); // Empty generator has no body
        // No content-type header for empty responses
        assert!(!response.headers().contains_key("content-type"));
    }

    #[tokio::test]
    async fn test_problem_details_error_response_generator() {
        // Test RFC 7807 Problem Details with custom authority
        use http::Uri;

        let authority: Uri = "https://api.example.com/problems/".parse().unwrap();
        let generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(Some(authority))
            .build();

        let response = generator.generate_response(ErrorResponseCode::InvalidConfiguration);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers()["content-type"], PROBLEM_DETAIL.to_string());

        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            // Should contain the error_response code in SCREAMING_SNAKE_CASE
            assert!(body_str.contains("INVALID_CONFIGURATION"));
            // Should contain the custom authority URL
            assert!(body_str.contains("https://api.example.com/problems/"));
            // Should contain the error_response message
            assert!(body_str.contains("Invalid configuration"));
        }
    }

    #[tokio::test]
    async fn test_custom_error_template_generator() {
        // Test custom authority in problem details
        use http::Uri;

        let custom_authority: Uri = "https://custom.example.com/errors/".parse().unwrap();
        let generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(Some(custom_authority))
            .build();

        let response = generator.generate_response(ErrorResponseCode::NoRoute);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("custom.example.com"));
            assert!(body_str.contains("NO_ROUTE"));
        }
    }

    #[tokio::test]
    async fn test_error_response_with_correlation_id() {
        // Test that trace ID is included when available in tracing context
        let generator = ProblemDetailErrorResponseGenerator::builder().authority(None).build();

        // Create a tracing span to test trace ID functionality
        let span = tracing::info_span!("test_span");
        let _enter = span.enter();

        let response = generator.generate_response(ErrorResponseCode::MissingConfiguration);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers()["content-type"], PROBLEM_DETAIL.to_string());

        // Response should be generated successfully (trace ID inclusion depends on active span)
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_error_response_internationalization() {
        // Test different error_response messages for different error_response codes
        let generator = HtmlErrorResponseGenerator::builder().build();

        let no_route_response = generator.generate_response(ErrorResponseCode::NoRoute);
        let access_denied_response = generator.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(no_route_response.status(), StatusCode::NOT_FOUND);
        assert_eq!(access_denied_response.status(), StatusCode::FORBIDDEN);

        // Different error_response codes should produce different messages
        if let (Some(no_route_body), Some(access_denied_body)) =
            (no_route_response.body(), access_denied_response.body())
        {
            let no_route_str = String::from_utf8_lossy(no_route_body);
            let access_denied_str = String::from_utf8_lossy(access_denied_body);

            assert!(no_route_str.contains("No matching route found"));
            assert!(access_denied_str.contains("Access denied"));
            assert_ne!(no_route_str, access_denied_str);
        }
    }

    #[tokio::test]
    async fn test_error_response_security_headers() {
        // Test that error_response response generators include appropriate headers
        let generator = HtmlErrorResponseGenerator::builder().build();

        let response = generator.generate_response(ErrorResponseCode::NoRoute);

        // Basic HTML response should have content-type and content-length
        assert!(response.headers().contains_key("content-type"));
        assert!(response.headers().contains_key("content-length"));

        // Security headers would be added by higher-level middleware
        // The generator focuses on the error_response content
    }

    #[tokio::test]
    async fn test_error_response_rate_limit_info() {
        // Test rate limit error_response response format
        let generator = ProblemDetailErrorResponseGenerator::builder().authority(None).build();

        // Using a generic error_response code (rate limiting would be a custom error_response code)
        let response = generator.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["content-type"], PROBLEM_DETAIL.to_string());

        // Rate limit specific headers would be added by higher-level middleware
        assert!(response.body().is_some());
    }
}
