use crate::filter::{
    error::FilterError,
    traits::{BackendRequestFilter, BackendResponseFilter, FilterHandler, PreBackendFilter},
    types::{FilterRequest, FilterResponse},
};
use http::{HeaderMap, HeaderName};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::Service;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;

/// Errors specific to HeaderModifier filter configuration
#[derive(Debug, Error)]
pub enum HeaderModifierError {
    #[error("Invalid header name: {name}")]
    InvalidHeaderName { name: String },

    #[error("Invalid header value: {value}")]
    InvalidHeaderValue { value: String },

    #[error("No header operations specified")]
    NoOperations,
}

// Automatic conversion from HeaderModifierError to unified FilterError
impl From<HeaderModifierError> for FilterError {
    fn from(err: HeaderModifierError) -> Self {
        FilterError::Configuration {
            message: err.to_string(),
        }
    }
}

/// Simplified HeaderModifierFilterHandler using Arc for data sharing
///
/// This implementation follows the new simplified architecture:
/// - Uses Arc for shared data instead of Clone requirement
/// - Implements Service trait directly with async blocks
/// - Supports both request and response header modification
/// - Uses TryFrom for configuration conversion
#[derive(Debug, Clone)]
pub struct HeaderModifierFilterHandler {
    add: Arc<HeaderMap>,
    set: Arc<HeaderMap>,
    remove: Arc<HashSet<HeaderName>>,
}

impl HeaderModifierFilterHandler {
    /// Create a new HeaderModifierFilterHandler with the given configuration
    pub fn new(add: HeaderMap, set: HeaderMap, remove: HashSet<HeaderName>) -> Self {
        Self {
            add: Arc::new(add),
            set: Arc::new(set),
            remove: Arc::new(remove),
        }
    }

    /// Apply header modifications to the given HeaderMap
    fn apply_modifications(&self, headers: &mut HeaderMap) {
        // Remove headers first
        for name in self.remove.iter() {
            headers.remove(name);
        }

        // Set headers (overwrite existing)
        for (name, value) in self.set.iter() {
            headers.insert(name, value.clone());
        }

        // Add headers (append to existing)
        for (name, value) in self.add.iter() {
            headers.append(name, value.clone());
        }
    }
}

// Implementation of Tower Service trait
impl Service<FilterRequest> for HeaderModifierFilterHandler {
    type Response = FilterResponse;
    type Error = FilterError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: FilterRequest) -> Self::Future {
        let (mut parts, _body) = req.into_parts();

        // Clone Arc references (cheap operation)
        let add = Arc::clone(&self.add);
        let set = Arc::clone(&self.set);
        let remove = Arc::clone(&self.remove);

        Box::pin(async move {
            // Apply header modifications in optimal order
            // 1. Remove headers first (most efficient)
            if !remove.is_empty() {
                for name in remove.iter() {
                    parts.headers.remove(name);
                }
            }

            // 2. Set headers (overwrites existing)
            if !set.is_empty() {
                for (name, value) in set.iter() {
                    parts.headers.insert(name, value.clone());
                }
            }

            // 3. Add headers (appends to existing)
            if !add.is_empty() {
                for (name, value) in add.iter() {
                    parts.headers.append(name, value.clone());
                }
            }

            // Create response with modified headers
            let mut response_builder = http::Response::builder().status(http::StatusCode::OK);

            // Copy headers from request to response
            for (name, value) in parts.headers.iter() {
                response_builder = response_builder.header(name, value);
            }

            let response =
                response_builder
                    .body(())
                    .map_err(|e| FilterError::HeaderModification {
                        message: format!("Failed to build response: {}", e),
                    })?;

            Ok(response)
        })
    }
}

// Implementation of FilterHandler trait
impl FilterHandler for HeaderModifierFilterHandler {
    type Config = HeaderModifierFilter;
    type ConfigError = HeaderModifierError;

    fn try_from_config(config: Self::Config) -> Result<Self, Self::ConfigError> {
        // Validate that at least one operation is specified
        if config.add().is_empty() && config.set().is_empty() && config.remove().is_empty() {
            return Err(HeaderModifierError::NoOperations);
        }

        // Validate header names and values
        for (name, value) in config.add().iter().chain(config.set().iter()) {
            if name.as_str().is_empty() {
                return Err(HeaderModifierError::InvalidHeaderName {
                    name: name.to_string(),
                });
            }
            if value.to_str().is_err() {
                return Err(HeaderModifierError::InvalidHeaderValue {
                    value: format!("{:?}", value),
                });
            }
        }

        for name in config.remove().iter() {
            if name.as_str().is_empty() {
                return Err(HeaderModifierError::InvalidHeaderName {
                    name: name.to_string(),
                });
            }
        }

        Ok(Self::new(
            config.add().clone(),
            config.set().clone(),
            config.remove().iter().cloned().collect(),
        ))
    }
}

// Implementation of stage-specific marker traits
impl PreBackendFilter for HeaderModifierFilterHandler {}
impl BackendRequestFilter for HeaderModifierFilterHandler {}
impl BackendResponseFilter for HeaderModifierFilterHandler {}

// TryFrom implementation for backward compatibility
impl TryFrom<HeaderModifierFilter> for HeaderModifierFilterHandler {
    type Error = HeaderModifierError;

    fn try_from(config: HeaderModifierFilter) -> Result<Self, Self::Error> {
        Self::try_from_config(config)
    }
}

// Legacy TryFrom implementation for references
impl TryFrom<&HeaderModifierFilter> for HeaderModifierFilterHandler {
    type Error = HeaderModifierError;

    fn try_from(config: &HeaderModifierFilter) -> Result<Self, Self::Error> {
        Self::try_from_config(config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[tokio::test]
    async fn test_header_addition() {
        // Test adding headers to requests
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Custom-Header", HeaderValue::from_static("custom-value"));
        add_headers.insert("X-Gateway-Version", HeaderValue::from_static("1.0"));

        let mut handler =
            HeaderModifierFilterHandler::new(add_headers, HeaderMap::new(), HashSet::new());

        let request = http::Request::builder()
            .uri("http://example.com")
            .body(())
            .unwrap();

        let response = handler.call(request).await.unwrap();
        let (parts, _) = response.into_parts();

        // Verify headers were added
        assert_eq!(
            parts.headers.get("x-custom-header").unwrap(),
            "custom-value"
        );
        assert_eq!(parts.headers.get("x-gateway-version").unwrap(), "1.0");
    }

    #[tokio::test]
    async fn test_header_removal() {
        // Test removing headers from requests
        let mut remove_headers = HashSet::new();
        remove_headers.insert(HeaderName::from_static("server"));
        remove_headers.insert(HeaderName::from_static("x-powered-by"));

        let mut handler =
            HeaderModifierFilterHandler::new(HeaderMap::new(), HeaderMap::new(), remove_headers);

        let request = http::Request::builder()
            .uri("http://example.com")
            .header("Server", "nginx/1.0")
            .header("X-Powered-By", "PHP/7.4")
            .header("Content-Type", "text/html")
            .body(())
            .unwrap();

        let response = handler.call(request).await.unwrap();
        let (parts, _) = response.into_parts();

        // Verify specified headers were removed
        assert!(parts.headers.get("server").is_none());
        assert!(parts.headers.get("x-powered-by").is_none());
        // Verify other headers remain
        assert_eq!(parts.headers.get("content-type").unwrap(), "text/html");
    }

    #[tokio::test]
    async fn test_header_modification() {
        // Test modifying existing headers using set operation
        let mut set_headers = HeaderMap::new();
        set_headers.insert("User-Agent", HeaderValue::from_static("Gateway-Proxy/1.0"));
        set_headers.insert("Server", HeaderValue::from_static("Vale-Gateway"));

        let mut handler =
            HeaderModifierFilterHandler::new(HeaderMap::new(), set_headers, HashSet::new());

        let request = http::Request::builder()
            .uri("http://example.com")
            .header("User-Agent", "Mozilla/5.0")
            .header("Content-Type", "text/html")
            .body(())
            .unwrap();

        let response = handler.call(request).await.unwrap();
        let (parts, _) = response.into_parts();

        // Verify headers were modified
        assert_eq!(
            parts.headers.get("user-agent").unwrap(),
            "Gateway-Proxy/1.0"
        );
        assert_eq!(parts.headers.get("server").unwrap(), "Vale-Gateway");
        // Verify unmodified headers remain
        assert_eq!(parts.headers.get("content-type").unwrap(), "text/html");
    }

    #[tokio::test]
    async fn test_combined_operations() {
        // Test all operations together: add, set, remove
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Added", HeaderValue::from_static("added-value"));

        let mut set_headers = HeaderMap::new();
        set_headers.insert("User-Agent", HeaderValue::from_static("Modified-Agent"));

        let mut remove_headers = HashSet::new();
        remove_headers.insert(HeaderName::from_static("x-remove-me"));

        let mut handler =
            HeaderModifierFilterHandler::new(add_headers, set_headers, remove_headers);

        let request = http::Request::builder()
            .uri("http://example.com")
            .header("User-Agent", "Original-Agent")
            .header("X-Remove-Me", "should-be-removed")
            .header("Content-Type", "application/json")
            .body(())
            .unwrap();

        let response = handler.call(request).await.unwrap();
        let (parts, _) = response.into_parts();

        // Verify all operations were applied
        assert_eq!(parts.headers.get("x-added").unwrap(), "added-value"); // Added
        assert_eq!(parts.headers.get("user-agent").unwrap(), "Modified-Agent"); // Set
        assert!(parts.headers.get("x-remove-me").is_none()); // Removed
        assert_eq!(
            parts.headers.get("content-type").unwrap(),
            "application/json"
        ); // Unchanged
    }

    #[test]
    fn test_try_from_config() {
        // Test TryFrom implementation with valid configuration
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Test", HeaderValue::from_static("test-value"));

        let mut set_headers = HeaderMap::new();
        set_headers.insert("User-Agent", HeaderValue::from_static("test-agent"));

        let remove_headers = vec![HeaderName::from_static("server")];

        let config = HeaderModifierFilter::builder()
            .add(add_headers)
            .set(set_headers)
            .remove(remove_headers)
            .build();

        let handler = HeaderModifierFilterHandler::try_from(config).unwrap();

        // Verify the handler was created correctly
        assert_eq!(handler.add.len(), 1);
        assert_eq!(handler.set.len(), 1);
        assert_eq!(handler.remove.len(), 1);
    }

    #[test]
    fn test_try_from_config_validation() {
        // Test validation of empty configuration
        let config = HeaderModifierFilter::builder()
            .add(HeaderMap::new())
            .set(HeaderMap::new())
            .remove(Vec::new())
            .build();

        let result = HeaderModifierFilterHandler::try_from(config);
        assert!(matches!(result, Err(HeaderModifierError::NoOperations)));
    }

    #[test]
    fn test_try_from_config_reference() {
        // Test TryFrom implementation with reference
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Test", HeaderValue::from_static("test-value"));

        let config = HeaderModifierFilter::builder()
            .add(add_headers)
            .set(HeaderMap::new())
            .remove(Vec::new())
            .build();

        let handler = HeaderModifierFilterHandler::try_from(&config).unwrap();
        assert_eq!(handler.add.len(), 1);
    }

    #[tokio::test]
    async fn test_service_trait_implementation() {
        // Test that the handler properly implements the Service trait
        let mut handler =
            HeaderModifierFilterHandler::new(HeaderMap::new(), HeaderMap::new(), HashSet::new());

        // Test poll_ready
        let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
        assert!(handler.poll_ready(&mut cx).is_ready());

        // Test call
        let request = http::Request::builder()
            .uri("http://example.com")
            .body(())
            .unwrap();

        let response = handler.call(request).await.unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_filter_handler_trait_implementation() {
        // Test FilterHandler trait implementation
        let config = HeaderModifierFilter::builder()
            .add({
                let mut headers = HeaderMap::new();
                headers.insert("X-Test", HeaderValue::from_static("test"));
                headers
            })
            .set(HeaderMap::new())
            .remove(Vec::new())
            .build();

        let handler = HeaderModifierFilterHandler::try_from_config(config).unwrap();
        assert_eq!(handler.add.len(), 1);
    }

    #[test]
    fn test_stage_marker_traits() {
        // Test that the handler implements the expected stage marker traits
        let handler =
            HeaderModifierFilterHandler::new(HeaderMap::new(), HeaderMap::new(), HashSet::new());

        // These should compile if the traits are implemented correctly
        fn _test_pre_backend_filter<T: PreBackendFilter>(_: T) {}
        fn _test_backend_request_filter<T: BackendRequestFilter>(_: T) {}
        fn _test_backend_response_filter<T: BackendResponseFilter>(_: T) {}

        _test_pre_backend_filter(handler.clone());
        _test_backend_request_filter(handler.clone());
        _test_backend_response_filter(handler);
    }

    #[tokio::test]
    async fn test_header_append_behavior() {
        // Test that add operation appends to existing headers
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Custom", HeaderValue::from_static("new-value"));

        let mut handler =
            HeaderModifierFilterHandler::new(add_headers, HeaderMap::new(), HashSet::new());

        let request = http::Request::builder()
            .uri("http://example.com")
            .header("X-Custom", "existing-value")
            .body(())
            .unwrap();

        let response = handler.call(request).await.unwrap();
        let (parts, _) = response.into_parts();

        // Should have both values (append behavior)
        let values: Vec<_> = parts.headers.get_all("x-custom").iter().collect();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&&HeaderValue::from_static("existing-value")));
        assert!(values.contains(&&HeaderValue::from_static("new-value")));
    }
}
