use crate::filter::{
    error::FilterError,
    traits::{
        BackendRequestFilter, BackendResponseFilter, FilterHandler, InboundRequestFilter,
        PreBackendFilter, ResponseGenerationFilter,
    },
    types::{FilterRequest, FilterResponse},
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Generic filter service that wraps an inner service with a filter
///
/// This service applies a filter to requests. The filter can either:
/// 1. Generate a response directly (e.g., access denied)
/// 2. Pass the request through to the inner service
///
/// The current implementation assumes filters generate responses directly.
/// In a future iteration, we may add support for request transformation
/// and passing to inner services.
pub struct FilterService<S, F> {
    inner: S,
    filter: Arc<F>,
}

impl<S, F> FilterService<S, F> {
    /// Create a new FilterService with the given inner service and filter
    pub fn new(inner: S, filter: Arc<F>) -> Self {
        Self { inner, filter }
    }

    /// Create a new FilterService from a filter without Arc wrapping
    pub fn from_filter(inner: S, filter: F) -> Self {
        Self {
            inner,
            filter: Arc::new(filter),
        }
    }

    /// Get a reference to the inner service
    pub fn inner(&self) -> &S {
        &self.inner
    }

    /// Get a reference to the filter
    pub fn filter(&self) -> &Arc<F> {
        &self.filter
    }

    /// Consume this service and return the inner service
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S, F> Service<FilterRequest> for FilterService<S, F>
where
    S: Service<FilterRequest, Response = FilterResponse, Error = FilterError> + Clone,
    F: FilterHandler<Response = FilterResponse, Error = FilterError> + Clone,
    F::Future: Send,
{
    type Response = FilterResponse;
    type Error = FilterError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Check readiness of the inner service
        // The filter is assumed to always be ready since it's stateless
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: FilterRequest) -> Self::Future {
        // Clone the filter for use in the async block
        // This is efficient because we're cloning an Arc
        let mut filter = (*self.filter).clone();

        // Use async block for simplified async patterns as required
        Box::pin(async move {
            // Apply the filter to the request
            let filter_response = filter.call(req).await?;

            // Handle service composition properly:
            // - If the filter generates a non-OK response (like 403, 404, etc.), return it directly
            // - If the filter generates an OK response, it indicates pass-through intent
            // - For now, we return the filter response directly as per current filter implementations
            //
            // Future enhancement: Check response status and conditionally pass to inner service
            // This would enable true middleware chaining where filters can modify requests
            // and pass them through to the next service in the chain
            // For example, if status is 200, we could pass through to inner service

            Ok(filter_response)
        })
    }
}

impl<S, F> Clone for FilterService<S, F>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            filter: Arc::clone(&self.filter),
        }
    }
}

// Macro to generate stage-specific layer types with reduced code duplication
macro_rules! impl_filter_layer {
    ($layer_name:ident, $filter_trait:ident) => {
        /// Stage-specific layer for type safety
        #[derive(Debug)]
        pub struct $layer_name<F> {
            filter: Arc<F>,
        }

        impl<F> $layer_name<F>
        where
            F: $filter_trait,
        {
            /// Create a new layer with the given filter
            pub fn new(filter: F) -> Self {
                Self {
                    filter: Arc::new(filter),
                }
            }

            /// Create a new layer with an already Arc-wrapped filter
            pub fn from_arc(filter: Arc<F>) -> Self {
                Self { filter }
            }

            /// Get a reference to the filter
            pub fn filter(&self) -> &Arc<F> {
                &self.filter
            }
        }

        impl<S, F> Layer<S> for $layer_name<F>
        where
            F: $filter_trait<Response = FilterResponse, Error = FilterError> + Clone,
            F::Future: Send,
            S: Service<FilterRequest, Response = FilterResponse, Error = FilterError> + Clone,
        {
            type Service = FilterService<S, F>;

            fn layer(&self, service: S) -> Self::Service {
                FilterService::new(service, Arc::clone(&self.filter))
            }
        }

        impl<F> Clone for $layer_name<F> {
            fn clone(&self) -> Self {
                Self {
                    filter: Arc::clone(&self.filter),
                }
            }
        }
    };
}

// Generate all stage-specific layer types
impl_filter_layer!(InboundRequestLayer, InboundRequestFilter);
impl_filter_layer!(PreBackendLayer, PreBackendFilter);
impl_filter_layer!(BackendRequestLayer, BackendRequestFilter);
impl_filter_layer!(BackendResponseLayer, BackendResponseFilter);
impl_filter_layer!(ResponseGenerationLayer, ResponseGenerationFilter);

#[cfg(test)]
mod tests {
    use super::*;
    use tower::service_fn;

    // Mock service for testing
    fn mock_service()
    -> impl Service<FilterRequest, Response = FilterResponse, Error = FilterError> + Clone {
        service_fn(|_req: FilterRequest| async {
            Ok(http::Response::builder().status(200).body(()).unwrap())
        })
    }

    // Mock filter for testing
    #[derive(Debug, Clone)]
    struct MockFilter;

    impl Service<FilterRequest> for MockFilter {
        type Response = FilterResponse;
        type Error = FilterError;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: FilterRequest) -> Self::Future {
            Box::pin(async move { Ok(http::Response::builder().status(200).body(()).unwrap()) })
        }
    }

    impl FilterHandler for MockFilter {
        type Config = ();
        type ConfigError = FilterError;

        fn try_from_config(_config: Self::Config) -> Result<Self, Self::ConfigError> {
            Ok(MockFilter)
        }
    }

    impl InboundRequestFilter for MockFilter {}

    #[tokio::test]
    async fn test_filter_service_composition() {
        let base_service = mock_service();
        let filter = MockFilter;

        // Test that we can create layers
        let inbound_layer = InboundRequestLayer::new(filter);

        // Test that layers can be applied to services
        let _composed_service = inbound_layer.layer(base_service);
    }

    #[test]
    fn test_layer_clone() {
        let filter = MockFilter;
        let layer = InboundRequestLayer::new(filter);
        let cloned_layer = layer.clone();

        // Verify that both layers point to the same filter instance
        assert!(Arc::ptr_eq(layer.filter(), cloned_layer.filter()));
    }
}
