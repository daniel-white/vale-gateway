use crate::filter::{
    error::FilterError,
    factory::StageServices,
    traits::{
        BackendRequestFilter, BackendResponseFilter, InboundRequestFilter, PreBackendFilter,
        ResponseGenerationFilter,
    },
    types::{FilterRequest, FilterResponse},
};

/// A collection of filter handlers organized by processing stage
///
/// This collection uses Vec<Box<dyn FilterTrait>> for type erasure as required,
/// allowing different filter types to be stored together while maintaining
/// type safety through stage-specific marker traits.
pub struct FilterCollection {
    /// Filters that process incoming requests before routing
    inbound_request: Vec<Box<dyn InboundRequestFilterTrait>>,

    /// Filters that process requests after routing but before backend
    pre_backend: Vec<Box<dyn PreBackendFilterTrait>>,

    /// Filters that process requests just before sending to backend
    backend_request: Vec<Box<dyn BackendRequestFilterTrait>>,

    /// Filters that process responses from backend
    backend_response: Vec<Box<dyn BackendResponseFilterTrait>>,

    /// Filters that generate responses without backend interaction
    response_generation: Vec<Box<dyn ResponseGenerationFilterTrait>>,
}

/// Trait object-safe version of InboundRequestFilter
///
/// This trait provides the interface needed for type erasure while maintaining
/// the functionality of the original trait. It uses Box<dyn Future> to make
/// the trait object-safe.
pub trait InboundRequestFilterTrait: Send + Sync {
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    >;
    fn clone_boxed(&self) -> Box<dyn InboundRequestFilterTrait>;
}

/// Trait object-safe version of PreBackendFilter
pub trait PreBackendFilterTrait: Send + Sync {
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    >;
    fn clone_boxed(&self) -> Box<dyn PreBackendFilterTrait>;
}

/// Trait object-safe version of BackendRequestFilter
pub trait BackendRequestFilterTrait: Send + Sync {
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    >;
    fn clone_boxed(&self) -> Box<dyn BackendRequestFilterTrait>;
}

/// Trait object-safe version of BackendResponseFilter
pub trait BackendResponseFilterTrait: Send + Sync {
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    >;
    fn clone_boxed(&self) -> Box<dyn BackendResponseFilterTrait>;
}

/// Trait object-safe version of ResponseGenerationFilter
pub trait ResponseGenerationFilterTrait: Send + Sync {
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    >;
    fn clone_boxed(&self) -> Box<dyn ResponseGenerationFilterTrait>;
}

// Implement trait object-safe versions for any type that implements the original traits
impl<T> InboundRequestFilterTrait for T
where
    T: InboundRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
    T::Future: Send + 'static,
{
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    > {
        let future = tower::Service::call(self, req);
        Box::pin(future)
    }

    fn clone_boxed(&self) -> Box<dyn InboundRequestFilterTrait> {
        Box::new(self.clone())
    }
}

impl<T> PreBackendFilterTrait for T
where
    T: PreBackendFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
    T::Future: Send + 'static,
{
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    > {
        let future = tower::Service::call(self, req);
        Box::pin(future)
    }

    fn clone_boxed(&self) -> Box<dyn PreBackendFilterTrait> {
        Box::new(self.clone())
    }
}

impl<T> BackendRequestFilterTrait for T
where
    T: BackendRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
    T::Future: Send + 'static,
{
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    > {
        let future = tower::Service::call(self, req);
        Box::pin(future)
    }

    fn clone_boxed(&self) -> Box<dyn BackendRequestFilterTrait> {
        Box::new(self.clone())
    }
}

impl<T> BackendResponseFilterTrait for T
where
    T: BackendResponseFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
    T::Future: Send + 'static,
{
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    > {
        let future = tower::Service::call(self, req);
        Box::pin(future)
    }

    fn clone_boxed(&self) -> Box<dyn BackendResponseFilterTrait> {
        Box::new(self.clone())
    }
}

impl<T> ResponseGenerationFilterTrait for T
where
    T: ResponseGenerationFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
    T::Future: Send + 'static,
{
    fn call_boxed(
        &mut self,
        req: FilterRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<FilterResponse, FilterError>> + Send>,
    > {
        let future = tower::Service::call(self, req);
        Box::pin(future)
    }

    fn clone_boxed(&self) -> Box<dyn ResponseGenerationFilterTrait> {
        Box::new(self.clone())
    }
}

impl FilterCollection {
    /// Create a new builder for FilterCollection
    pub fn builder() -> FilterCollectionBuilder {
        FilterCollectionBuilder::new()
    }

    /// Create a new empty FilterCollection
    pub fn new() -> Self {
        Self {
            inbound_request: Vec::new(),
            pre_backend: Vec::new(),
            backend_request: Vec::new(),
            backend_response: Vec::new(),
            response_generation: Vec::new(),
        }
    }

    /// Add an inbound request filter to the collection
    ///
    /// This is a convenient method for adding filters after collection creation.
    pub fn add_inbound_request<F>(mut self, filter: F) -> Self
    where
        F: InboundRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.inbound_request.push(Box::new(filter));
        self
    }

    /// Add a pre-backend filter to the collection
    pub fn add_pre_backend<F>(mut self, filter: F) -> Self
    where
        F: PreBackendFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.pre_backend.push(Box::new(filter));
        self
    }

    /// Add a backend request filter to the collection
    pub fn add_backend_request<F>(mut self, filter: F) -> Self
    where
        F: BackendRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.backend_request.push(Box::new(filter));
        self
    }

    /// Add a backend response filter to the collection
    pub fn add_backend_response<F>(mut self, filter: F) -> Self
    where
        F: BackendResponseFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.backend_response.push(Box::new(filter));
        self
    }

    /// Add a response generation filter to the collection
    pub fn add_response_generation<F>(mut self, filter: F) -> Self
    where
        F: ResponseGenerationFilter<Response = FilterResponse, Error = FilterError>
            + Clone
            + 'static,
        F::Future: Send + 'static,
    {
        self.response_generation.push(Box::new(filter));
        self
    }

    /// Build services from the filter collection
    ///
    /// This method creates Tower services for each stage using the FilterServiceFactory.
    /// It provides the bridge between filter configuration and service creation.
    pub fn build_services(self) -> Result<StageServices, FilterError> {
        // For now, we'll create empty services as a placeholder
        // The challenge is converting from trait objects back to concrete types
        // for service creation. This would require a different approach in a full implementation.
        Ok(StageServices::empty())
    }

    /// Get the number of filters in each stage
    pub fn stage_counts(&self) -> StageCounts {
        StageCounts {
            inbound_request: self.inbound_request.len(),
            pre_backend: self.pre_backend.len(),
            backend_request: self.backend_request.len(),
            backend_response: self.backend_response.len(),
            response_generation: self.response_generation.len(),
        }
    }

    /// Check if the collection is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inbound_request.is_empty()
            && self.pre_backend.is_empty()
            && self.backend_request.is_empty()
            && self.backend_response.is_empty()
            && self.response_generation.is_empty()
    }

    /// Get the total number of filters across all stages
    #[inline]
    pub fn total_count(&self) -> usize {
        self.inbound_request.len()
            + self.pre_backend.len()
            + self.backend_request.len()
            + self.backend_response.len()
            + self.response_generation.len()
    }

    /// Get references to the filter collections for each stage
    pub fn inbound_request_filters(&self) -> &[Box<dyn InboundRequestFilterTrait>] {
        &self.inbound_request
    }

    pub fn pre_backend_filters(&self) -> &[Box<dyn PreBackendFilterTrait>] {
        &self.pre_backend
    }

    pub fn backend_request_filters(&self) -> &[Box<dyn BackendRequestFilterTrait>] {
        &self.backend_request
    }

    pub fn backend_response_filters(&self) -> &[Box<dyn BackendResponseFilterTrait>] {
        &self.backend_response
    }

    pub fn response_generation_filters(&self) -> &[Box<dyn ResponseGenerationFilterTrait>] {
        &self.response_generation
    }
}

/// Information about the number of filters in each stage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCounts {
    pub inbound_request: usize,
    pub pre_backend: usize,
    pub backend_request: usize,
    pub backend_response: usize,
    pub response_generation: usize,
}

impl StageCounts {
    pub fn total(&self) -> usize {
        self.inbound_request
            + self.pre_backend
            + self.backend_request
            + self.backend_response
            + self.response_generation
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Builder for creating FilterCollection instances
///
/// This builder provides convenient methods for adding filters to each stage
/// and building the final collection.
pub struct FilterCollectionBuilder {
    inbound_request: Vec<Box<dyn InboundRequestFilterTrait>>,
    pre_backend: Vec<Box<dyn PreBackendFilterTrait>>,
    backend_request: Vec<Box<dyn BackendRequestFilterTrait>>,
    backend_response: Vec<Box<dyn BackendResponseFilterTrait>>,
    response_generation: Vec<Box<dyn ResponseGenerationFilterTrait>>,
}

impl FilterCollectionBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            inbound_request: Vec::new(),
            pre_backend: Vec::new(),
            backend_request: Vec::new(),
            backend_response: Vec::new(),
            response_generation: Vec::new(),
        }
    }

    /// Add an inbound request filter
    pub fn add_inbound_request<F>(mut self, filter: F) -> Self
    where
        F: InboundRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.inbound_request.push(Box::new(filter));
        self
    }

    /// Add a pre-backend filter
    pub fn add_pre_backend<F>(mut self, filter: F) -> Self
    where
        F: PreBackendFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.pre_backend.push(Box::new(filter));
        self
    }

    /// Add a backend request filter
    pub fn add_backend_request<F>(mut self, filter: F) -> Self
    where
        F: BackendRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.backend_request.push(Box::new(filter));
        self
    }

    /// Add a backend response filter
    pub fn add_backend_response<F>(mut self, filter: F) -> Self
    where
        F: BackendResponseFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        self.backend_response.push(Box::new(filter));
        self
    }

    /// Add a response generation filter
    pub fn add_response_generation<F>(mut self, filter: F) -> Self
    where
        F: ResponseGenerationFilter<Response = FilterResponse, Error = FilterError>
            + Clone
            + 'static,
        F::Future: Send + 'static,
    {
        self.response_generation.push(Box::new(filter));
        self
    }

    /// Build the FilterCollection
    pub fn build(self) -> FilterCollection {
        FilterCollection {
            inbound_request: self.inbound_request,
            pre_backend: self.pre_backend,
            backend_request: self.backend_request,
            backend_response: self.backend_response,
            response_generation: self.response_generation,
        }
    }
}

impl Default for FilterCollectionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    use tower::Service;

    // Mock filter for testing
    #[derive(Debug, Clone)]
    struct MockFilter {
        id: u32,
        status: http::StatusCode,
    }

    impl MockFilter {
        fn new(id: u32) -> Self {
            Self {
                id,
                status: http::StatusCode::OK,
            }
        }

        fn with_status(id: u32, status: http::StatusCode) -> Self {
            Self { id, status }
        }
    }

    impl Service<FilterRequest> for MockFilter {
        type Response = FilterResponse;
        type Error = FilterError;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: FilterRequest) -> Self::Future {
            let status = self.status;
            Box::pin(async move {
                Ok(http::Response::builder()
                    .status(status)
                    .body(())
                    .expect("Failed to build response"))
            })
        }
    }

    impl crate::filter::traits::FilterHandler for MockFilter {
        type Config = u32;
        type ConfigError = FilterError;

        fn try_from_config(config: Self::Config) -> Result<Self, Self::ConfigError> {
            Ok(MockFilter::new(config))
        }
    }

    impl InboundRequestFilter for MockFilter {}
    impl PreBackendFilter for MockFilter {}
    impl BackendRequestFilter for MockFilter {}
    impl BackendResponseFilter for MockFilter {}
    impl ResponseGenerationFilter for MockFilter {}

    #[test]
    fn test_filter_collection_builder() {
        let collection = FilterCollection::builder()
            .add_inbound_request(MockFilter::new(1))
            .add_pre_backend(MockFilter::new(2))
            .add_backend_request(MockFilter::new(3))
            .add_backend_response(MockFilter::new(4))
            .add_response_generation(MockFilter::new(5))
            .build();

        let counts = collection.stage_counts();
        assert_eq!(counts.inbound_request, 1);
        assert_eq!(counts.pre_backend, 1);
        assert_eq!(counts.backend_request, 1);
        assert_eq!(counts.backend_response, 1);
        assert_eq!(counts.response_generation, 1);
        assert_eq!(counts.total(), 5);
        assert!(!collection.is_empty());
    }

    #[test]
    fn test_filter_collection_add_methods() {
        let collection = FilterCollection::builder()
            .build()
            .add_inbound_request(MockFilter::new(1))
            .add_pre_backend(MockFilter::new(2));

        let counts = collection.stage_counts();
        assert_eq!(counts.inbound_request, 1);
        assert_eq!(counts.pre_backend, 1);
        assert_eq!(counts.total(), 2);
    }

    #[test]
    fn test_empty_filter_collection() {
        let collection = FilterCollection::builder().build();

        assert!(collection.is_empty());
        assert_eq!(collection.total_count(), 0);

        let counts = collection.stage_counts();
        assert!(counts.is_empty());
        assert_eq!(counts.total(), 0);
    }

    #[test]
    fn test_filter_collection_multiple_filters_per_stage() {
        let collection = FilterCollection::builder()
            .add_inbound_request(MockFilter::new(1))
            .add_inbound_request(MockFilter::new(2))
            .add_inbound_request(MockFilter::new(3))
            .add_pre_backend(MockFilter::new(4))
            .add_pre_backend(MockFilter::new(5))
            .build();

        let counts = collection.stage_counts();
        assert_eq!(counts.inbound_request, 3);
        assert_eq!(counts.pre_backend, 2);
        assert_eq!(counts.total(), 5);
    }

    #[test]
    fn test_stage_counts() {
        let counts = StageCounts {
            inbound_request: 2,
            pre_backend: 1,
            backend_request: 0,
            backend_response: 3,
            response_generation: 1,
        };

        assert_eq!(counts.total(), 7);
        assert!(!counts.is_empty());

        let empty_counts = StageCounts {
            inbound_request: 0,
            pre_backend: 0,
            backend_request: 0,
            backend_response: 0,
            response_generation: 0,
        };

        assert_eq!(empty_counts.total(), 0);
        assert!(empty_counts.is_empty());
    }

    #[test]
    fn test_filter_collection_access_methods() {
        let collection = FilterCollection::builder()
            .add_inbound_request(MockFilter::new(1))
            .add_pre_backend(MockFilter::new(2))
            .build();

        // Test that we can access the filter collections
        assert_eq!(collection.inbound_request_filters().len(), 1);
        assert_eq!(collection.pre_backend_filters().len(), 1);
        assert_eq!(collection.backend_request_filters().len(), 0);
        assert_eq!(collection.backend_response_filters().len(), 0);
        assert_eq!(collection.response_generation_filters().len(), 0);
    }

    #[tokio::test]
    async fn test_build_services() {
        let collection = FilterCollection::builder()
            .add_inbound_request(MockFilter::new(1))
            .build();

        // Test that we can build services from the collection
        let _services = collection.build_services().unwrap();

        // For now, this just tests that the method works
        // In a full implementation, we would test the actual service functionality
    }
}
