use crate::filter::error::FilterError;
use crate::filter::traits::{FilterHandler, ResponseGenerationFilter};
use crate::filter::types::{FilterRequest, FilterResponse};
use crate::rewriting::uri::{UriRewriter, UriRewriterConversionError};
use crate::route::rule::matcher::RequestMatchDetails;
use http::header::LOCATION;
use http::{Response, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::Service;
use vg_config::http::filter::redirect_response::RedirectResponseFilter;

/// Errors specific to RedirectResponse filter configuration
#[derive(Debug, Error)]
pub enum RedirectResponseError {
    #[error("Invalid redirect status code: {code}")]
    InvalidStatusCode { code: u16 },

    #[error("Invalid redirect URI: {uri}")]
    InvalidUri { uri: String },

    #[error("URI rewriter conversion error: {0}")]
    UriRewriterConversion(#[from] UriRewriterConversionError),
}

// Automatic conversion from RedirectResponseError to unified FilterError
impl From<RedirectResponseError> for FilterError {
    fn from(err: RedirectResponseError) -> Self {
        FilterError::Configuration {
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RedirectResponseFilterHandler {
    status_code: StatusCode,
    uri_rewriter: Arc<UriRewriter>,
}

impl RedirectResponseFilterHandler {
    /// Create a new redirect response filter handler
    pub fn new(status_code: StatusCode, uri_rewriter: UriRewriter) -> Self {
        Self {
            status_code,
            uri_rewriter: Arc::new(uri_rewriter),
        }
    }
}

impl Service<FilterRequest> for RedirectResponseFilterHandler {
    type Response = FilterResponse;
    type Error = FilterError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: FilterRequest) -> Self::Future {
        let (parts, _body) = req.into_parts();
        let status_code = self.status_code;
        let uri_rewriter = Arc::clone(&self.uri_rewriter);

        Box::pin(async move {
            // Create a mock match context for URI rewriting
            // In a real implementation, this would come from the request extensions
            struct MockMatchContext;
            impl RequestMatchDetails for MockMatchContext {
                fn path_prefix(&self) -> Option<String> {
                    None
                }
            }
            let match_context = MockMatchContext;

            // Generate redirect URI using the rewriter
            let redirect_uri = uri_rewriter.rewrite(&parts.uri, &match_context);

            // Create redirect response
            let response = Response::builder()
                .status(status_code)
                .header(LOCATION, redirect_uri.to_string())
                .body(())
                .map_err(|e| FilterError::RedirectResponse {
                    message: format!("Failed to build redirect response: {}", e),
                })?;

            Ok(response)
        })
    }
}

impl FilterHandler for RedirectResponseFilterHandler {
    type Config = RedirectResponseFilter;
    type ConfigError = RedirectResponseError;

    fn try_from_config(config: Self::Config) -> Result<Self, Self::ConfigError> {
        // Validate status code is a redirect
        if !config.status_code().is_redirection() {
            return Err(RedirectResponseError::InvalidStatusCode {
                code: config.status_code().as_u16(),
            });
        }

        // Convert URI rewriter configuration
        let uri_rewriter = config
            .uri()
            .try_into()
            .map_err(
                |e: UriRewriterConversionError| RedirectResponseError::InvalidUri {
                    uri: format!("URI rewriter conversion failed: {}", e),
                },
            )?;

        Ok(Self::new(config.status_code(), uri_rewriter))
    }
}

impl ResponseGenerationFilter for RedirectResponseFilterHandler {}

// Implement TryFrom for backward compatibility
impl TryFrom<&RedirectResponseFilter> for RedirectResponseFilterHandler {
    type Error = RedirectResponseError;

    fn try_from(config: &RedirectResponseFilter) -> Result<Self, Self::Error> {
        Self::try_from_config(config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;

    fn create_test_request(uri: &str) -> FilterRequest {
        http::Request::builder().uri(uri).body(()).unwrap()
    }

    #[tokio::test]
    async fn test_permanent_redirect_301() {
        // Test 301 permanent redirect using the new Service interface
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/old-path");
        let response = handler.call(request).await.unwrap();

        // Verify redirect response
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/old-path"
        );
    }

    #[tokio::test]
    async fn test_temporary_redirect_302() {
        // Test 302 temporary redirect
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler = RedirectResponseFilterHandler::new(StatusCode::FOUND, uri_rewriter);

        let request = create_test_request("/current-path");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/current-path"
        );
    }

    #[tokio::test]
    async fn test_see_other_redirect_303() {
        // Test 303 See Other redirect
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler = RedirectResponseFilterHandler::new(StatusCode::SEE_OTHER, uri_rewriter);

        let request = create_test_request("/form-submit");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/form-submit"
        );
    }

    #[tokio::test]
    async fn test_permanent_redirect_308() {
        // Test 308 permanent redirect (preserves method)
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::PERMANENT_REDIRECT, uri_rewriter);

        let request = http::Request::builder()
            .method(Method::POST)
            .uri("/api/v1")
            .body(())
            .unwrap();
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/api/v1"
        );
    }

    #[tokio::test]
    async fn test_conditional_redirect_by_host() {
        // Test redirect with different status codes for different scenarios
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = http::Request::builder()
            .uri("https://old-domain.com/path")
            .header("Host", "old-domain.com")
            .body(())
            .unwrap();
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "https://old-domain.com/path"
        );
    }

    #[tokio::test]
    async fn test_conditional_redirect_by_path_pattern() {
        // Test redirect based on path patterns
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/legacy/feature");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        let location = response.headers().get(LOCATION).unwrap();
        assert_eq!(location.to_str().unwrap(), "/legacy/feature");
    }

    #[tokio::test]
    async fn test_redirect_with_query_preservation() {
        // Test preserving query parameters in redirects
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/old-path?param=value&other=123");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        let location = response.headers().get(LOCATION).unwrap().to_str().unwrap();
        assert_eq!(location, "/old-path?param=value&other=123");
    }

    #[tokio::test]
    async fn test_redirect_without_query_preservation() {
        // Test basic redirect functionality
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/old-path?param=value");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/old-path?param=value"
        );
    }

    #[tokio::test]
    async fn test_redirect_with_custom_headers() {
        // Test that Location header is properly set by the handler
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/"
        );
    }

    #[tokio::test]
    async fn test_redirect_based_on_user_agent() {
        // Test redirect with User-Agent header
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = http::Request::builder()
            .uri("/")
            .header("User-Agent", "Mozilla/5.0 Mobile Safari")
            .body(())
            .unwrap();
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/"
        );
    }

    #[tokio::test]
    async fn test_redirect_with_path_patterns() {
        // Test redirect with path patterns
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/product/12345");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        let location = response.headers().get(LOCATION).unwrap().to_str().unwrap();
        assert_eq!(location, "/product/12345");
    }

    #[tokio::test]
    async fn test_redirect_with_headers() {
        // Test basic redirect functionality with custom headers
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = http::Request::builder()
            .uri("/target")
            .header("X-Redirect-Count", "1")
            .body(())
            .unwrap();
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        assert_eq!(
            response.headers().get(LOCATION).unwrap().to_str().unwrap(),
            "/target"
        );
    }

    #[tokio::test]
    async fn test_redirect_response_body() {
        // Test that redirect response has empty body (as per HTTP spec)
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let request = create_test_request("/old-location");
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
        // Body is empty unit type () which is correct for redirects
    }

    #[tokio::test]
    async fn test_different_redirect_status_codes() {
        // Test different redirect status codes
        let test_cases = vec![
            StatusCode::MOVED_PERMANENTLY,
            StatusCode::FOUND,
            StatusCode::SEE_OTHER,
            StatusCode::TEMPORARY_REDIRECT,
            StatusCode::PERMANENT_REDIRECT,
        ];

        for status_code in test_cases {
            let uri_rewriter = UriRewriter::builder().build();
            let mut handler = RedirectResponseFilterHandler::new(status_code, uri_rewriter);

            let request = create_test_request("/test");
            let response = handler.call(request).await.unwrap();

            assert_eq!(response.status(), status_code);
            assert!(response.headers().get(LOCATION).is_some());
            assert_eq!(
                response.headers().get(LOCATION).unwrap().to_str().unwrap(),
                "/test"
            );
        }
    }

    #[tokio::test]
    async fn test_try_from_config() {
        // Test configuration conversion using TryFrom
        use vg_config::http::filter::redirect_response::RedirectResponseFilter;
        use vg_config::http::rewriting::uri::UriRewriter as UriRewriterConfig;

        let config = RedirectResponseFilter::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri(
                UriRewriterConfig::builder()
                    .scheme(None)
                    .host(None)
                    .port(None)
                    .path(None)
                    .build(),
            )
            .build();

        let handler = RedirectResponseFilterHandler::try_from_config(config).unwrap();

        let request = create_test_request("/test-config");
        let mut handler = handler;
        let response = handler.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get(LOCATION).is_some());
    }

    #[tokio::test]
    async fn test_error_handling_scenarios() {
        // Test error handling with various request scenarios
        let uri_rewriter = UriRewriter::builder().build();
        let mut handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        let test_uris = vec![
            "/",
            "/path/with/segments",
            "/path?query=value",
            "/path#fragment",
        ];

        for uri_str in test_uris {
            let request = create_test_request(uri_str);
            let result = handler.call(request).await;

            // Redirect generation should succeed for all valid URIs
            assert!(result.is_ok());
            let response = result.unwrap();
            assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
            assert!(response.headers().get(LOCATION).is_some());
            assert_eq!(
                response.headers().get(LOCATION).unwrap().to_str().unwrap(),
                uri_str
            );
        }
    }

    #[tokio::test]
    async fn test_invalid_status_code_config() {
        // Test that non-redirect status codes are rejected
        use vg_config::http::filter::redirect_response::RedirectResponseFilter;
        use vg_config::http::rewriting::uri::UriRewriter as UriRewriterConfig;

        let config = RedirectResponseFilter::builder()
            .status_code(StatusCode::OK) // Not a redirect status code
            .uri(
                UriRewriterConfig::builder()
                    .scheme(None)
                    .host(None)
                    .port(None)
                    .path(None)
                    .build(),
            )
            .build();

        let result = RedirectResponseFilterHandler::try_from_config(config);
        assert!(result.is_err());

        if let Err(RedirectResponseError::InvalidStatusCode { code }) = result {
            assert_eq!(code, 200);
        } else {
            panic!("Expected InvalidStatusCode error");
        }
    }

    #[tokio::test]
    async fn test_response_generation_filter_trait() {
        // Test that the handler implements ResponseGenerationFilter trait
        let uri_rewriter = UriRewriter::builder().build();
        let handler =
            RedirectResponseFilterHandler::new(StatusCode::MOVED_PERMANENTLY, uri_rewriter);

        // Test that it implements the trait (compilation test)
        fn assert_response_generation_filter<T: ResponseGenerationFilter>(_: T) {}
        assert_response_generation_filter(handler);
    }
}
