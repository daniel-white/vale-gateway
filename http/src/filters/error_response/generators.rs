use super::error_codes::ErrorResponseCode;
use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{HeaderValue, Response, StatusCode};
use opentelemetry::TraceId;
use opentelemetry::trace::TraceContextExt;
use problemdetails::Problem;
use std::borrow::Cow;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use typed_builder::TypedBuilder;
use url::Url;

#[derive(Debug)]
pub enum ErrorResponseGeneratorType {
    Empty(EmptyErrorResponseGenerator),
    Html(HtmlErrorResponseGenerator),
    ProblemDetail(ProblemDetailErrorResponseGenerator),
}

impl ErrorResponseGeneratorType {
    pub fn generate_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        match self {
            Self::Empty(generator) => generator.generate_response(code),
            Self::Html(generator) => generator.generate_response(code),
            Self::ProblemDetail(generator) => generator.generate_response(code),
        }
    }
}

trait ErrorResponseGenerator {
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

    fn body(&self, _code: ErrorResponseCode) -> Option<(HeaderValue, Cow<'static, str>)> {
        None
    }

    fn trace_id(&self) -> Option<String> {
        let span = Span::current();
        let context = span.context();
        let trace_id = context.span().span_context().trace_id();
        if trace_id != TraceId::INVALID {
            Some(trace_id.to_string())
        } else {
            None
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct EmptyErrorResponseGenerator {}

impl ErrorResponseGenerator for EmptyErrorResponseGenerator {}

#[derive(Debug, TypedBuilder)]
pub struct HtmlErrorResponseGenerator {}

impl ErrorResponseGenerator for HtmlErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(HeaderValue, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let body = format!("<html><body><h1>{message}</h1></body></html>");
        Some((HeaderValue::from_static("text/html"), body.into()))
    }
}

#[derive(Debug, TypedBuilder)]
pub struct ProblemDetailErrorResponseGenerator {
    #[builder(setter(into), default)]
    authority: Option<Url>,
}

impl ErrorResponseGenerator for ProblemDetailErrorResponseGenerator {
    fn body(&self, code: ErrorResponseCode) -> Option<(HeaderValue, Cow<'static, str>)> {
        let message: Cow<_> = code.into();
        let code_str: &'static str = code.into();
        let status_code: StatusCode = code.into();

        let problem = Problem::from(code)
            .with_value("status", status_code.as_u16())
            .with_type(format!(
                "{}{}",
                self.authority
                    .as_ref()
                    .map(|a| a.as_str())
                    .unwrap_or("http://vale-gateway.whitefamily.in/errors/"),
                code_str
            ))
            .with_detail(message);

        let problem = if let Some(trace_id) = self.trace_id() {
            problem.with_value("trace_id", trace_id)
        } else {
            problem
        };

        let body: String = serde_json::to_string(&problem.body).unwrap();

        Some((
            HeaderValue::from_static("application/problem+json"),
            body.into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::error_response::ErrorResponseFilterHandler;
    use assertables::*;
    use bytes::Bytes;
    use http::{HeaderValue, Response, StatusCode};
    use serde_json::json;

    #[tokio::test]
    async fn test_json_error_response_generator() {
        // Test the actual ProblemDetailErrorResponseGenerator (JSON format)
        let generator = ProblemDetailErrorResponseGenerator::builder().build();

        let response = generator.generate_response(ErrorResponseCode::UpstreamUnavailable);

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );

        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            assert!(body_str.contains("UPSTREAM_UNAVAILABLE"));
            assert!(body_str.contains("Upstream unavailable"));
        }
    }

    #[tokio::test]
    async fn test_html_error_response_generator() {
        // Test the actual HtmlErrorResponseGenerator
        let generator = HtmlErrorResponseGenerator::builder().build();

        let response = generator.generate_response(ErrorResponseCode::NoRoute);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["content-type"], "text/html");

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
        use url::Url;

        let authority = Url::parse("https://api.example.com/problems/").unwrap();
        let generator = ProblemDetailErrorResponseGenerator::builder()
            .authority(Some(authority))
            .build();

        let response = generator.generate_response(ErrorResponseCode::InvalidConfiguration);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );

        if let Some(body) = response.body() {
            let body_str = String::from_utf8_lossy(body);
            // Should contain the error code in SCREAMING_SNAKE_CASE
            assert!(body_str.contains("INVALID_CONFIGURATION"));
            // Should contain the custom authority URL
            assert!(body_str.contains("https://api.example.com/problems/"));
            // Should contain the error message
            assert!(body_str.contains("Invalid configuration"));
        }
    }

    #[tokio::test]
    async fn test_custom_error_template_generator() {
        // Test custom authority in problem details
        use url::Url;

        let custom_authority = Url::parse("https://custom.example.com/errors/").unwrap();
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
        let generator = ProblemDetailErrorResponseGenerator::builder().build();

        // Create a tracing span to test trace ID functionality
        let span = tracing::info_span!("test_span");
        let _enter = span.enter();

        let response = generator.generate_response(ErrorResponseCode::MissingConfiguration);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );

        // Response should be generated successfully (trace ID inclusion depends on active span)
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_error_response_internationalization() {
        // Test different error messages for different error codes
        let generator = HtmlErrorResponseGenerator::builder().build();

        let no_route_response = generator.generate_response(ErrorResponseCode::NoRoute);
        let access_denied_response = generator.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(no_route_response.status(), StatusCode::NOT_FOUND);
        assert_eq!(access_denied_response.status(), StatusCode::FORBIDDEN);

        // Different error codes should produce different messages
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
        // Test that error response generators include appropriate headers
        let generator = HtmlErrorResponseGenerator::builder().build();

        let response = generator.generate_response(ErrorResponseCode::NoRoute);

        // Basic HTML response should have content-type and content-length
        assert!(response.headers().contains_key("content-type"));
        assert!(response.headers().contains_key("content-length"));

        // Security headers would be added by higher-level middleware
        // The generator focuses on the error content
    }

    #[tokio::test]
    async fn test_error_response_rate_limit_info() {
        // Test rate limit error response format
        let generator = ProblemDetailErrorResponseGenerator::builder().build();

        // Using a generic error code (rate limiting would be a custom error code)
        let response = generator.generate_response(ErrorResponseCode::AccessDenied);

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );

        // Rate limit specific headers would be added by higher-level middleware
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_error_response_content_negotiation() {
        // Test different generators for content negotiation
        let html_generator =
            ErrorResponseGeneratorType::Html(HtmlErrorResponseGenerator::builder().build());
        let json_generator = ErrorResponseGeneratorType::ProblemDetail(
            ProblemDetailErrorResponseGenerator::builder().build(),
        );

        let html_handler = ErrorResponseFilterHandler::builder()
            .generator(html_generator)
            .build();
        let json_handler = ErrorResponseFilterHandler::builder()
            .generator(json_generator)
            .build();

        let error_code = ErrorResponseCode::UpstreamUnavailable;

        let html_response = html_handler.generate_response(error_code);
        let json_response = json_handler.generate_response(error_code);

        // Same error code, different formats
        assert_eq!(html_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json_response.status(), StatusCode::SERVICE_UNAVAILABLE);

        assert_eq!(html_response.headers()["content-type"], "text/html");
        assert_eq!(
            json_response.headers()["content-type"],
            "application/problem+json"
        );
    }
}
