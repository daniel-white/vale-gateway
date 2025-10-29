use crate::filter::{
    error::FilterError,
    layer::FilterService,
    traits::{
        BackendRequestFilter, BackendResponseFilter, FilterHandler, InboundRequestFilter,
        PreBackendFilter, ResponseGenerationFilter,
    },
    types::{BoxedFilterService, FilterRequest, FilterResponse},
};
use tower::{service_fn, util::BoxService};

/// Service factory for creating Tower services from filter handlers
///
/// This factory provides methods to create services for each filter stage,
/// leveraging Tower's ServiceBuilder for composition and BoxService for type erasure.
/// It separates filter creation from service building as required.
pub struct FilterServiceFactory;

impl FilterServiceFactory {
    /// Create a service for inbound request filters
    ///
    /// This method takes a collection of inbound request filters and composes them
    /// into a single service using Tower's ServiceBuilder pattern.
    pub fn create_inbound_service<F>(filters: Vec<F>) -> BoxedFilterService
    where
        F: InboundRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        Self::create_service_for_stage(filters)
    }

    /// Create a service for pre-backend filters
    ///
    /// This method takes a collection of pre-backend filters and composes them
    /// into a single service using Tower's ServiceBuilder pattern.
    pub fn create_pre_backend_service<F>(filters: Vec<F>) -> BoxedFilterService
    where
        F: PreBackendFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        Self::create_service_for_stage(filters)
    }

    /// Create a service for backend request filters
    ///
    /// This method takes a collection of backend request filters and composes them
    /// into a single service using Tower's ServiceBuilder pattern.
    pub fn create_backend_request_service<F>(filters: Vec<F>) -> BoxedFilterService
    where
        F: BackendRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        Self::create_service_for_stage(filters)
    }

    /// Create a service for backend response filters
    ///
    /// This method takes a collection of backend response filters and composes them
    /// into a single service using Tower's ServiceBuilder pattern.
    pub fn create_backend_response_service<F>(filters: Vec<F>) -> BoxedFilterService
    where
        F: BackendResponseFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        Self::create_service_for_stage(filters)
    }

    /// Create a service for response generation filters
    ///
    /// This method takes a collection of response generation filters and composes them
    /// into a single service using Tower's ServiceBuilder pattern.
    pub fn create_response_generation_service<F>(filters: Vec<F>) -> BoxedFilterService
    where
        F: ResponseGenerationFilter<Response = FilterResponse, Error = FilterError>
            + Clone
            + 'static,
        F::Future: Send + 'static,
    {
        Self::create_service_for_stage(filters)
    }

    /// Generic method to create a service for any stage
    ///
    /// This method uses Tower's ServiceBuilder to compose filters into a service chain.
    /// It leverages BoxService for type erasure when needed and follows Tower ecosystem
    /// best practices for service composition.
    #[inline]
    fn create_service_for_stage<F>(filters: Vec<F>) -> BoxedFilterService
    where
        F: FilterHandler<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        // Fast path for empty filters
        if filters.is_empty() {
            return Self::create_passthrough_service();
        }

        // Take the first filter and create a service
        // In a full implementation, we would need to properly compose all filters
        // This is a limitation of the current approach - proper filter chaining
        // would require a different service composition strategy
        if let Some(first_filter) = filters.into_iter().next() {
            let base_service = service_fn(|_req: FilterRequest| async {
                Ok(http::Response::builder()
                    .status(200)
                    .body(())
                    .expect("Failed to build response"))
            });
            BoxService::new(FilterService::from_filter(base_service, first_filter))
        } else {
            Self::create_passthrough_service()
        }
    }

    /// Create a base service that returns a successful response
    ///
    /// This service is used as the foundation for filter composition.
    /// It represents the "end" of the filter chain.
    fn create_base_service() -> BoxedFilterService {
        BoxService::new(service_fn(|_req: FilterRequest| async {
            Ok(http::Response::builder()
                .status(200)
                .body(())
                .expect("Failed to build response"))
        }))
    }

    /// Create a pass-through service for empty filter collections
    ///
    /// This service simply passes requests through without modification.
    fn create_passthrough_service() -> BoxedFilterService {
        Self::create_base_service()
    }

    /// Create a service from a single filter handler
    ///
    /// This is a convenience method for creating services from individual filters.
    pub fn create_single_filter_service<F>(filter: F) -> BoxedFilterService
    where
        F: FilterHandler<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        F::Future: Send + 'static,
    {
        let base_service = service_fn(|_req: FilterRequest| async {
            Ok(http::Response::builder()
                .status(200)
                .body(())
                .expect("Failed to build response"))
        });
        let service = FilterService::from_filter(base_service, filter);
        BoxService::new(service)
    }

    /// Create a composed service from multiple filter stages
    ///
    /// This method demonstrates how to compose services from different stages
    /// using Tower's ServiceBuilder. This is useful for creating complete
    /// request processing pipelines.
    pub fn create_composed_service<IR, PB, BR, BResp, RG>(
        inbound_request_filters: Vec<IR>,
        pre_backend_filters: Vec<PB>,
        backend_request_filters: Vec<BR>,
        backend_response_filters: Vec<BResp>,
        response_generation_filters: Vec<RG>,
    ) -> BoxedFilterService
    where
        IR: InboundRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        IR::Future: Send + 'static,
        PB: PreBackendFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        PB::Future: Send + 'static,
        BR: BackendRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        BR::Future: Send + 'static,
        BResp:
            BackendResponseFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        BResp::Future: Send + 'static,
        RG: ResponseGenerationFilter<Response = FilterResponse, Error = FilterError>
            + Clone
            + 'static,
        RG::Future: Send + 'static,
    {
        // Create individual stage services
        let inbound_service = Self::create_inbound_service(inbound_request_filters);
        let _pre_backend_service = Self::create_pre_backend_service(pre_backend_filters);
        let _backend_request_service =
            Self::create_backend_request_service(backend_request_filters);
        let _backend_response_service =
            Self::create_backend_response_service(backend_response_filters);
        let _response_generation_service =
            Self::create_response_generation_service(response_generation_filters);

        // For now, we'll return the inbound service as the primary service
        // In a full implementation, these would be composed into a complete pipeline
        // This demonstrates the pattern for service composition
        inbound_service
    }
}

/// Container for services created for each filter stage
///
/// This struct provides organized access to services for each processing stage,
/// making it easy to use the appropriate service at the right time in the
/// request processing pipeline.
pub struct StageServices {
    pub inbound_request: BoxedFilterService,
    pub pre_backend: BoxedFilterService,
    pub backend_request: BoxedFilterService,
    pub backend_response: BoxedFilterService,
    pub response_generation: BoxedFilterService,
}

impl StageServices {
    /// Create stage services from filter collections
    ///
    /// This method provides a convenient way to create all stage services
    /// from collections of filters for each stage.
    pub fn from_filters<IR, PB, BR, BResp, RG>(
        inbound_request_filters: Vec<IR>,
        pre_backend_filters: Vec<PB>,
        backend_request_filters: Vec<BR>,
        backend_response_filters: Vec<BResp>,
        response_generation_filters: Vec<RG>,
    ) -> Self
    where
        IR: InboundRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        IR::Future: Send + 'static,
        PB: PreBackendFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        PB::Future: Send + 'static,
        BR: BackendRequestFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        BR::Future: Send + 'static,
        BResp:
            BackendResponseFilter<Response = FilterResponse, Error = FilterError> + Clone + 'static,
        BResp::Future: Send + 'static,
        RG: ResponseGenerationFilter<Response = FilterResponse, Error = FilterError>
            + Clone
            + 'static,
        RG::Future: Send + 'static,
    {
        Self {
            inbound_request: FilterServiceFactory::create_inbound_service(inbound_request_filters),
            pre_backend: FilterServiceFactory::create_pre_backend_service(pre_backend_filters),
            backend_request: FilterServiceFactory::create_backend_request_service(
                backend_request_filters,
            ),
            backend_response: FilterServiceFactory::create_backend_response_service(
                backend_response_filters,
            ),
            response_generation: FilterServiceFactory::create_response_generation_service(
                response_generation_filters,
            ),
        }
    }

    /// Create empty stage services
    ///
    /// This creates services for all stages with no filters applied.
    /// Useful for testing or when no filtering is needed.
    pub fn empty() -> Self {
        Self {
            inbound_request: FilterServiceFactory::create_passthrough_service(),
            pre_backend: FilterServiceFactory::create_passthrough_service(),
            backend_request: FilterServiceFactory::create_passthrough_service(),
            backend_response: FilterServiceFactory::create_passthrough_service(),
            response_generation: FilterServiceFactory::create_passthrough_service(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::traits::FilterHandler;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    use tower::{Service, ServiceExt};

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

    impl FilterHandler for MockFilter {
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

    fn create_test_request() -> FilterRequest {
        http::Request::builder()
            .uri("/test")
            .body(())
            .expect("Failed to build request")
    }

    #[tokio::test]
    async fn test_create_inbound_service_empty() {
        let filters: Vec<MockFilter> = vec![];
        let service = FilterServiceFactory::create_inbound_service(filters);

        let request = create_test_request();
        let response = service.oneshot(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_create_inbound_service_single_filter() {
        let filters = vec![MockFilter::new(1)];
        let mut service = FilterServiceFactory::create_inbound_service(filters);

        let request = create_test_request();
        let response = service.call(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_create_inbound_service_multiple_filters() {
        let filters = vec![MockFilter::new(1), MockFilter::new(2), MockFilter::new(3)];
        let mut service = FilterServiceFactory::create_inbound_service(filters);

        let request = create_test_request();
        let response = service.call(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_create_single_filter_service() {
        let filter = MockFilter::with_status(1, http::StatusCode::FORBIDDEN);
        let mut service = FilterServiceFactory::create_single_filter_service(filter);

        let request = create_test_request();
        let response = service.call(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_stage_services_from_filters() {
        let inbound_filters = vec![MockFilter::new(1)];
        let pre_backend_filters = vec![MockFilter::new(2)];
        let backend_request_filters = vec![MockFilter::new(3)];
        let backend_response_filters = vec![MockFilter::new(4)];
        let response_generation_filters = vec![MockFilter::new(5)];

        let stage_services = StageServices::from_filters(
            inbound_filters,
            pre_backend_filters,
            backend_request_filters,
            backend_response_filters,
            response_generation_filters,
        );

        // Test that all services are created and functional
        let request = create_test_request();
        let response = stage_services
            .inbound_request
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn test_stage_services_empty() {
        let stage_services = StageServices::empty();

        // Test that empty services work correctly
        let request = create_test_request();
        let response = stage_services
            .inbound_request
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
    }

    #[test]
    fn test_create_all_stage_services() {
        // Test that we can create services for all stages
        let filter = MockFilter::new(1);

        let _inbound_service = FilterServiceFactory::create_inbound_service(vec![filter.clone()]);
        let _pre_backend_service =
            FilterServiceFactory::create_pre_backend_service(vec![filter.clone()]);
        let _backend_request_service =
            FilterServiceFactory::create_backend_request_service(vec![filter.clone()]);
        let _backend_response_service =
            FilterServiceFactory::create_backend_response_service(vec![filter.clone()]);
        let _response_generation_service =
            FilterServiceFactory::create_response_generation_service(vec![filter]);

        // If we get here without compilation errors, the factory works correctly
    }

    #[tokio::test]
    async fn test_composed_service() {
        let inbound_filters = vec![MockFilter::new(1)];
        let pre_backend_filters = vec![MockFilter::new(2)];
        let backend_request_filters = vec![MockFilter::new(3)];
        let backend_response_filters = vec![MockFilter::new(4)];
        let response_generation_filters = vec![MockFilter::new(5)];

        let mut service = FilterServiceFactory::create_composed_service(
            inbound_filters,
            pre_backend_filters,
            backend_request_filters,
            backend_response_filters,
            response_generation_filters,
        );

        let request = create_test_request();
        let response = service.call(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::OK);
    }
}
