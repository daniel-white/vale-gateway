use crate::filter::error::FilterError;
use crate::filter::traits::{FilterHandler, ResponseGenerationFilter};
use crate::filter::types::{FilterRequest, FilterResponse, FilterResult};
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{HeaderValue, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::Service;
use vg_config::http::filter::static_response::{
    Body as BodyConfig, BodyContent as BodyContentConfig, StaticResponseFilter,
};
use vg_core::http::content_type::ContentTypeBuf;

/// Errors specific to StaticResponse filter configuration
#[derive(Debug, Error)]
pub enum StaticResponseError {
    #[error("Invalid status code: {code}")]
    InvalidStatusCode { code: u16 },

    #[error("Invalid content type: {content_type}")]
    InvalidContentType { content_type: String },

    #[error("Body resolution error: {message}")]
    BodyResolution { message: String },
}

// Automatic conversion from StaticResponseError to unified FilterError
impl From<StaticResponseError> for FilterError {
    fn from(err: StaticResponseError) -> Self {
        FilterError::Configuration {
            message: err.to_string(),
        }
    }
}

/// Simplified StaticResponseFilterHandler using Arc for data sharing
///
/// This implementation follows the new simplified pattern:
/// - Uses Arc for shared data instead of Clone
/// - Implements Service trait directly
/// - Uses simplified async patterns
/// - Integrates with unified error handling
#[derive(Debug, Clone)]
pub struct StaticResponseFilterHandler {
    status_code: StatusCode,
    body_content: Option<Arc<BodyContent>>,
}

/// Body content stored with Arc for efficient sharing
#[derive(Debug)]
pub struct BodyContent {
    content_type: ContentTypeBuf,
    data: Arc<[u8]>,
}

impl StaticResponseFilterHandler {
    /// Create a new StaticResponseFilterHandler
    pub fn new(status_code: StatusCode, body_content: Option<Arc<BodyContent>>) -> Self {
        Self {
            status_code,
            body_content,
        }
    }

    /// Generate a static response
    fn generate_response(&self) -> FilterResult<FilterResponse> {
        let mut builder = http::Response::builder().status(self.status_code);

        if let Some(body) = &self.body_content {
            // Add content type and length headers
            let content_type: HeaderValue =
                (&body.content_type)
                    .try_into()
                    .map_err(|e| FilterError::StaticResponse {
                        message: format!("Invalid content type: {}", e),
                    })?;

            builder = builder
                .header(CONTENT_TYPE, content_type)
                .header(CONTENT_LENGTH, body.data.len());
        }

        let response = builder.body(()).map_err(|e| FilterError::StaticResponse {
            message: format!("Failed to build response: {}", e),
        })?;

        Ok(response)
    }
}

impl BodyContent {
    /// Create new body content from configuration
    pub fn from_config(config: &BodyConfig) -> Result<Self, StaticResponseError> {
        let data = match config.content() {
            BodyContentConfig::Text(text) => Arc::from(text.as_bytes()),
            BodyContentConfig::Binary(data) => data.clone(),
            BodyContentConfig::Remote(_uri) => {
                return Err(StaticResponseError::BodyResolution {
                    message: "Remote body content not supported in simplified implementation"
                        .to_string(),
                });
            }
        };

        Ok(Self {
            content_type: config.content_type().clone(),
            data,
        })
    }
}

// Implement Service trait for StaticResponseFilterHandler
impl Service<FilterRequest> for StaticResponseFilterHandler {
    type Response = FilterResponse;
    type Error = FilterError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: FilterRequest) -> Self::Future {
        let response_result = self.generate_response();

        Box::pin(async move { response_result })
    }
}

// Implement FilterHandler trait
impl FilterHandler for StaticResponseFilterHandler {
    type Config = StaticResponseFilter;
    type ConfigError = StaticResponseError;

    fn try_from_config(config: Self::Config) -> Result<Self, Self::ConfigError> {
        let body_content = match config.body() {
            Some(body_config) => Some(Arc::new(BodyContent::from_config(body_config)?)),
            None => None,
        };

        Ok(Self::new(config.status_code(), body_content))
    }
}

// Implement ResponseGenerationFilter marker trait
impl ResponseGenerationFilter for StaticResponseFilterHandler {}

// Implement TryFrom for backward compatibility
impl TryFrom<StaticResponseFilter> for StaticResponseFilterHandler {
    type Error = StaticResponseError;

    fn try_from(config: StaticResponseFilter) -> Result<Self, Self::Error> {
        Self::try_from_config(config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_static_response_without_body() {
        // Create a StaticResponseFilterHandler without body
        let mut handler = StaticResponseFilterHandler::new(StatusCode::NOT_FOUND, None);

        // Create a test request
        let request = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(())
            .unwrap();

        // Call the service
        let result = handler.call(request).await;

        // Verify the response
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(!response.headers().contains_key(CONTENT_TYPE));
        assert!(!response.headers().contains_key(CONTENT_LENGTH));
    }

    #[tokio::test]
    async fn test_static_response_with_text_body() {
        // Create body content
        let body_content = Arc::new(BodyContent {
            content_type: "text/plain".parse().unwrap(),
            data: Arc::from("Hello, World!".as_bytes()),
        });

        // Create a StaticResponseFilterHandler with body
        let mut handler = StaticResponseFilterHandler::new(StatusCode::OK, Some(body_content));

        // Create a test request
        let request = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(())
            .unwrap();

        // Call the service
        let result = handler.call(request).await;

        // Verify the response
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "text/plain");
        assert_eq!(response.headers().get(CONTENT_LENGTH).unwrap(), "13");
    }

    #[tokio::test]
    async fn test_try_from_config_without_body() {
        // Create configuration without body
        let config = StaticResponseFilter::builder()
            .status_code(StatusCode::NOT_FOUND)
            .body(None)
            .build();

        // Test TryFrom conversion
        let result = StaticResponseFilterHandler::try_from(config);
        assert!(result.is_ok());

        let handler = result.unwrap();
        assert_eq!(handler.status_code, StatusCode::NOT_FOUND);
        assert!(handler.body_content.is_none());
    }

    #[tokio::test]
    async fn test_try_from_config_with_text_body() {
        // Create body configuration
        let body_config = BodyConfig::builder()
            .content_type("text/html".parse::<ContentTypeBuf>().unwrap())
            .content(BodyContentConfig::Text("<!DOCTYPE html>".to_string()))
            .build();

        // Create configuration with body
        let config = StaticResponseFilter::builder()
            .status_code(StatusCode::OK)
            .body(Some(body_config))
            .build();

        // Test TryFrom conversion
        let result = StaticResponseFilterHandler::try_from(config);
        assert!(result.is_ok());

        let handler = result.unwrap();
        assert_eq!(handler.status_code, StatusCode::OK);
        assert!(handler.body_content.is_some());

        let body = handler.body_content.unwrap();
        assert_eq!(body.content_type.to_string(), "text/html");
        assert_eq!(&*body.data, "<!DOCTYPE html>".as_bytes());
    }

    #[tokio::test]
    async fn test_try_from_config_with_binary_body() {
        // Create binary body configuration
        let binary_data: Arc<[u8]> = Arc::from([0x89, 0x50, 0x4E, 0x47].as_slice()); // PNG header
        let body_config = BodyConfig::builder()
            .content_type("image/png".parse::<ContentTypeBuf>().unwrap())
            .content(BodyContentConfig::Binary(binary_data.clone()))
            .build();

        // Create configuration with binary body
        let config = StaticResponseFilter::builder()
            .status_code(StatusCode::OK)
            .body(Some(body_config))
            .build();

        // Test TryFrom conversion
        let result = StaticResponseFilterHandler::try_from(config);
        assert!(result.is_ok());

        let handler = result.unwrap();
        assert_eq!(handler.status_code, StatusCode::OK);
        assert!(handler.body_content.is_some());

        let body = handler.body_content.unwrap();
        assert_eq!(body.content_type.to_string(), "image/png");
        assert_eq!(&*body.data, &*binary_data);
    }

    #[tokio::test]
    async fn test_try_from_config_with_remote_body_error() {
        // Create remote body configuration (should fail)
        let body_config = BodyConfig::builder()
            .content_type("text/plain".parse::<ContentTypeBuf>().unwrap())
            .content(BodyContentConfig::Remote(
                "http://example.com/test".parse().unwrap(),
            ))
            .build();

        // Create configuration with remote body
        let config = StaticResponseFilter::builder()
            .status_code(StatusCode::OK)
            .body(Some(body_config))
            .build();

        // Test TryFrom conversion should fail
        let result = StaticResponseFilterHandler::try_from(config);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(matches!(error, StaticResponseError::BodyResolution { .. }));
        assert!(
            error
                .to_string()
                .contains("Remote body content not supported")
        );
    }

    #[tokio::test]
    async fn test_filter_handler_trait_implementation() {
        // Test that StaticResponseFilterHandler implements FilterHandler
        fn assert_filter_handler<T: FilterHandler>(_: &T) {}

        let handler = StaticResponseFilterHandler::new(StatusCode::OK, None);
        assert_filter_handler(&handler);
    }

    #[tokio::test]
    async fn test_response_generation_filter_trait_implementation() {
        // Test that StaticResponseFilterHandler implements ResponseGenerationFilter
        fn assert_response_generation_filter<T: ResponseGenerationFilter>(_: &T) {}

        let handler = StaticResponseFilterHandler::new(StatusCode::OK, None);
        assert_response_generation_filter(&handler);
    }

    #[tokio::test]
    async fn test_service_trait_implementation() {
        // Test that StaticResponseFilterHandler implements Service
        let handler = StaticResponseFilterHandler::new(StatusCode::OK, None);

        // Test oneshot call
        let request = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(())
            .unwrap();

        let result = handler.oneshot(request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
