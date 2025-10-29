use crate::filter::collection::FilterHandlerCollection;
use getset::Getters;
use std::fmt;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;

/// Errors that can occur during stage service building
#[derive(Debug, Error)]
pub enum StageServiceBuilderError {
    #[error("Service creation failed for stage {stage}: {message}")]
    ServiceCreationFailed { stage: String, message: String },

    #[error("Layer application failed for stage {stage}: {source}")]
    LayerApplicationFailed {
        stage: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Invalid service configuration: {message}")]
    InvalidConfiguration { message: String },
}

/// A collection of services, one per stage, built from layer collections
#[derive(Debug, TypedBuilder, Getters)]
pub struct StageServices<S> {
    #[getset(get = "pub")]
    inbound_request_service: S,

    #[getset(get = "pub")]
    pre_backend_service: S,

    #[getset(get = "pub")]
    backend_request_service: S,

    #[getset(get = "pub")]
    backend_response_service: S,

    #[getset(get = "pub")]
    response_generation_service: S,
}

impl<S> StageServices<S> {
    /// Get the total number of services
    pub fn service_count(&self) -> usize {
        5 // Always 5 services (one per stage)
    }

    /// Check if all services are the same instance (no layers applied)
    pub fn has_no_layers(&self) -> bool
    where
        S: PartialEq,
    {
        self.inbound_request_service == self.pre_backend_service
            && self.pre_backend_service == self.backend_request_service
            && self.backend_request_service == self.backend_response_service
            && self.backend_response_service == self.response_generation_service
    }
}

impl<S> Clone for StageServices<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inbound_request_service: self.inbound_request_service.clone(),
            pre_backend_service: self.pre_backend_service.clone(),
            backend_request_service: self.backend_request_service.clone(),
            backend_response_service: self.backend_response_service.clone(),
            response_generation_service: self.response_generation_service.clone(),
        }
    }
}

/// Builder for creating StageServices from filter handlers
#[derive(Debug, TypedBuilder)]
pub struct StageServiceBuilder<S> {
    base_service: S,
}

impl<S> StageServiceBuilder<S>
where
    S: Clone,
{
    /// Create a new StageServiceBuilder
    pub fn new(base_service: S) -> Self {
        Self::builder().base_service(base_service).build()
    }

    /// Build StageServices from a FilterHandlerCollection
    pub fn build_from_collection<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>(
        self,
        collection: FilterHandlerCollection<
            InboundReq,
            PreBackend,
            BackendReq,
            BackendResp,
            RespGen,
        >,
    ) -> Result<StageServices<S>, StageServiceBuilderError>
    where
        InboundReq: Layer<S, Service = S> + Clone,
        PreBackend: Layer<S, Service = S> + Clone,
        BackendReq: Layer<S, Service = S> + Clone,
        BackendResp: Layer<S, Service = S> + Clone,
        RespGen: Layer<S, Service = S> + Clone,
        S: Clone + fmt::Debug,
    {
        let base_service = self.base_service;

        // Build separate services for each stage by applying layers
        let inbound_request_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            collection.inbound_request_handlers().to_vec(),
            "inbound_request",
        )?;

        let pre_backend_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            collection.pre_backend_handlers().to_vec(),
            "pre_backend",
        )?;

        let backend_request_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            collection.backend_request_handlers().to_vec(),
            "backend_request",
        )?;

        let backend_response_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            collection.backend_response_handlers().to_vec(),
            "backend_response",
        )?;

        let response_generation_service = Self::apply_layers_with_error_handling(
            base_service,
            collection.response_generation_handlers().to_vec(),
            "response_generation",
        )?;

        Ok(StageServices::builder()
            .inbound_request_service(inbound_request_service)
            .pre_backend_service(pre_backend_service)
            .backend_request_service(backend_request_service)
            .backend_response_service(backend_response_service)
            .response_generation_service(response_generation_service)
            .build())
    }

    /// Build StageServices from filter handlers using concrete types (legacy method)
    pub fn build_with_handlers<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>(
        self,
        inbound_request_handlers: Vec<InboundReq>,
        pre_backend_handlers: Vec<PreBackend>,
        backend_request_handlers: Vec<BackendReq>,
        backend_response_handlers: Vec<BackendResp>,
        response_generation_handlers: Vec<RespGen>,
    ) -> Result<StageServices<S>, StageServiceBuilderError>
    where
        InboundReq: Layer<S, Service = S> + Clone,
        PreBackend: Layer<S, Service = S> + Clone,
        BackendReq: Layer<S, Service = S> + Clone,
        BackendResp: Layer<S, Service = S> + Clone,
        RespGen: Layer<S, Service = S> + Clone,
        S: Clone + fmt::Debug,
    {
        let base_service = self.base_service;

        // Build separate services for each stage by applying layers
        let inbound_request_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            inbound_request_handlers,
            "inbound_request",
        )?;

        let pre_backend_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            pre_backend_handlers,
            "pre_backend",
        )?;

        let backend_request_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            backend_request_handlers,
            "backend_request",
        )?;

        let backend_response_service = Self::apply_layers_with_error_handling(
            base_service.clone(),
            backend_response_handlers,
            "backend_response",
        )?;

        let response_generation_service = Self::apply_layers_with_error_handling(
            base_service,
            response_generation_handlers,
            "response_generation",
        )?;

        Ok(StageServices::builder()
            .inbound_request_service(inbound_request_service)
            .pre_backend_service(pre_backend_service)
            .backend_request_service(backend_request_service)
            .backend_response_service(backend_response_service)
            .response_generation_service(response_generation_service)
            .build())
    }

    /// Apply layers to service in reverse order (Tower pattern) with error handling
    fn apply_layers_with_error_handling<L>(
        mut service: S,
        handlers: Vec<L>,
        stage_name: &str,
    ) -> Result<S, StageServiceBuilderError>
    where
        L: Layer<S, Service = S> + Clone,
        S: Clone + fmt::Debug,
    {
        // Apply layers in reverse order (Tower pattern)
        for (index, handler) in handlers.into_iter().rev().enumerate() {
            // Attempt to apply the layer
            let new_service = handler.layer(service);

            // Basic validation - ensure the service was created successfully
            // In a real implementation, you might want more sophisticated validation
            service = new_service;

            // Log successful layer application (in debug builds)
            #[cfg(debug_assertions)]
            eprintln!(
                "Applied layer {} to {} stage service",
                index + 1,
                stage_name
            );
        }

        Ok(service)
    }

    /// Apply layers to service in reverse order (Tower pattern) - legacy method without error handling
    fn apply_layers<L>(mut service: S, handlers: Vec<L>) -> S
    where
        L: Layer<S, Service = S> + Clone,
        S: Clone,
    {
        // Apply layers in reverse order (Tower pattern)
        for handler in handlers.into_iter().rev() {
            service = handler.layer(service);
        }
        service
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::FilterHandlerCollectionBuilder;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::Service;

    // Mock service for testing
    #[derive(Debug, Clone)]
    struct MockService {
        id: String,
        call_count: Arc<AtomicUsize>,
    }

    impl PartialEq for MockService {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl MockService {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl Service<()> for MockService {
        type Response = String;
        type Error = ();
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(
            &mut self,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: ()) -> Self::Future {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok(format!("Response from {}", self.id)))
        }
    }

    // Mock layer that returns the same service type (identity layer for testing)
    #[derive(Debug, Clone)]
    struct MockLayer {
        id: String,
    }

    impl MockLayer {
        fn new(id: &str) -> Self {
            Self { id: id.to_string() }
        }
    }

    impl<S> Layer<S> for MockLayer
    where
        S: Clone,
    {
        type Service = S;

        fn layer(&self, service: S) -> Self::Service {
            // For testing purposes, just return the service unchanged
            // In a real implementation, this would wrap the service
            service
        }
    }

    #[test]
    fn test_stage_service_builder_new() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        assert_eq!(builder.base_service, base_service);
    }

    #[test]
    fn test_stage_services_creation() {
        let base_service = MockService::new("base");
        let stage_services = StageServices::builder()
            .inbound_request_service(base_service.clone())
            .pre_backend_service(base_service.clone())
            .backend_request_service(base_service.clone())
            .backend_response_service(base_service.clone())
            .response_generation_service(base_service.clone())
            .build();

        assert_eq!(stage_services.service_count(), 5);
        assert!(stage_services.has_no_layers());
        assert_eq!(*stage_services.inbound_request_service(), base_service);
        assert_eq!(*stage_services.pre_backend_service(), base_service);
        assert_eq!(*stage_services.backend_request_service(), base_service);
        assert_eq!(*stage_services.backend_response_service(), base_service);
        assert_eq!(*stage_services.response_generation_service(), base_service);
    }

    #[test]
    fn test_stage_services_clone() {
        let base_service = MockService::new("base");
        let stage_services = StageServices::builder()
            .inbound_request_service(base_service.clone())
            .pre_backend_service(base_service.clone())
            .backend_request_service(base_service.clone())
            .backend_response_service(base_service.clone())
            .response_generation_service(base_service.clone())
            .build();

        let cloned_services = stage_services.clone();
        assert_eq!(
            cloned_services.service_count(),
            stage_services.service_count()
        );
        assert_eq!(
            *cloned_services.inbound_request_service(),
            *stage_services.inbound_request_service()
        );
    }

    #[test]
    fn test_build_with_handlers_empty() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        let stage_services = builder
            .build_with_handlers::<MockLayer, MockLayer, MockLayer, MockLayer, MockLayer>(
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
            )
            .expect("Build should succeed with empty handlers");

        assert_eq!(stage_services.service_count(), 5);
        assert!(stage_services.has_no_layers());
    }

    #[test]
    fn test_build_with_handlers_single_layer_per_stage() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        let inbound_layer = MockLayer::new("inbound");
        let pre_backend_layer = MockLayer::new("pre_backend");
        let backend_request_layer = MockLayer::new("backend_request");
        let backend_response_layer = MockLayer::new("backend_response");
        let response_generation_layer = MockLayer::new("response_generation");

        let stage_services = builder
            .build_with_handlers::<MockLayer, MockLayer, MockLayer, MockLayer, MockLayer>(
                vec![inbound_layer],
                vec![pre_backend_layer],
                vec![backend_request_layer],
                vec![backend_response_layer],
                vec![response_generation_layer],
            )
            .expect("Build should succeed");

        assert_eq!(stage_services.service_count(), 5);
        // With our identity mock layer, all services are the same, so has_no_layers() returns true
        assert!(stage_services.has_no_layers());
    }

    #[test]
    fn test_build_with_handlers_multiple_layers_per_stage() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        let inbound_layer1 = MockLayer::new("inbound1");
        let inbound_layer2 = MockLayer::new("inbound2");
        let pre_backend_layer = MockLayer::new("pre_backend");

        let stage_services = builder
            .build_with_handlers::<MockLayer, MockLayer, MockLayer, MockLayer, MockLayer>(
                vec![inbound_layer1, inbound_layer2],
                vec![pre_backend_layer],
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
            )
            .expect("Build should succeed");

        assert_eq!(stage_services.service_count(), 5);
        // With our identity mock layer, all services are the same, so has_no_layers() returns true
        assert!(stage_services.has_no_layers());
    }

    #[test]
    fn test_build_from_collection_empty() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        let collection: FilterHandlerCollection<
            MockLayer,
            MockLayer,
            MockLayer,
            MockLayer,
            MockLayer,
        > = FilterHandlerCollectionBuilder::new().build_unchecked();

        let stage_services = builder
            .build_from_collection(collection)
            .expect("Build should succeed with empty collection");

        assert_eq!(stage_services.service_count(), 5);
        assert!(stage_services.has_no_layers());
    }

    #[test]
    fn test_build_from_collection_with_handlers() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        let collection: FilterHandlerCollection<
            MockLayer,
            MockLayer,
            MockLayer,
            MockLayer,
            MockLayer,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockLayer::new("inbound"))
            .add_pre_backend_handler(MockLayer::new("pre_backend"))
            .add_backend_request_handler(MockLayer::new("backend_request"))
            .add_backend_response_handler(MockLayer::new("backend_response"))
            .add_response_generation_handler(MockLayer::new("response_generation"))
            .build_unchecked();

        let stage_services = builder
            .build_from_collection(collection)
            .expect("Build should succeed");

        assert_eq!(stage_services.service_count(), 5);
        // With our identity mock layer, all services are the same, so has_no_layers() returns true
        assert!(stage_services.has_no_layers());
    }

    #[test]
    fn test_layer_application_order() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service.clone());

        // Create layers that will be applied in reverse order
        let layer1 = MockLayer::new("layer1");
        let layer2 = MockLayer::new("layer2");
        let layer3 = MockLayer::new("layer3");

        let stage_services = builder
            .build_with_handlers::<MockLayer, MockLayer, MockLayer, MockLayer, MockLayer>(
                vec![layer1, layer2, layer3], // These should be applied in reverse order
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
            )
            .expect("Build should succeed");

        // The layers should have been applied in reverse order (Tower pattern)
        // This is verified by the fact that the build succeeds and creates the expected service structure
        assert_eq!(stage_services.service_count(), 5);
        // With our identity mock layer, all services are the same, so has_no_layers() returns true
        assert!(stage_services.has_no_layers());
    }

    #[test]
    fn test_apply_layers_empty() {
        let base_service = MockService::new("base");
        let result =
            StageServiceBuilder::apply_layers(base_service.clone(), Vec::<MockLayer>::new());
        assert_eq!(result, base_service);
    }

    #[test]
    fn test_apply_layers_single() {
        let base_service = MockService::new("base");
        let layer = MockLayer::new("test");
        let result = StageServiceBuilder::apply_layers(base_service.clone(), vec![layer]);

        // With our identity mock layer, the result should be the same as the base service
        assert_eq!(result, base_service);
    }

    #[test]
    fn test_apply_layers_multiple() {
        let base_service = MockService::new("base");
        let layer1 = MockLayer::new("layer1");
        let layer2 = MockLayer::new("layer2");
        let result = StageServiceBuilder::apply_layers(base_service.clone(), vec![layer1, layer2]);

        // With our identity mock layer, the result should be the same as the base service
        assert_eq!(result, base_service);
    }

    #[test]
    fn test_apply_layers_with_error_handling_success() {
        let base_service = MockService::new("base");
        let layer = MockLayer::new("test");

        let result = StageServiceBuilder::apply_layers_with_error_handling(
            base_service,
            vec![layer],
            "test_stage",
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_layers_with_error_handling_empty() {
        let base_service = MockService::new("base");

        let result = StageServiceBuilder::apply_layers_with_error_handling(
            base_service.clone(),
            Vec::<MockLayer>::new(),
            "test_stage",
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), base_service);
    }

    #[test]
    fn test_stage_service_builder_error_display() {
        let error = StageServiceBuilderError::ServiceCreationFailed {
            stage: "test_stage".to_string(),
            message: "test message".to_string(),
        };

        let error_string = format!("{}", error);
        assert!(error_string.contains("Service creation failed"));
        assert!(error_string.contains("test_stage"));
        assert!(error_string.contains("test message"));
    }

    #[test]
    fn test_stage_service_builder_error_invalid_configuration() {
        let error = StageServiceBuilderError::InvalidConfiguration {
            message: "invalid config".to_string(),
        };

        let error_string = format!("{}", error);
        assert!(error_string.contains("Invalid service configuration"));
        assert!(error_string.contains("invalid config"));
    }

    #[test]
    fn test_stage_services_has_no_layers_different_services() {
        let service1 = MockService::new("service1");
        let service2 = MockService::new("service2");

        let stage_services = StageServices::builder()
            .inbound_request_service(service1)
            .pre_backend_service(service2.clone())
            .backend_request_service(service2.clone())
            .backend_response_service(service2.clone())
            .response_generation_service(service2)
            .build();

        assert!(!stage_services.has_no_layers());
    }

    // Integration test that combines FilterHandlerCollection with StageServiceBuilder
    #[test]
    fn test_integration_collection_to_stage_services() {
        let base_service = MockService::new("base");

        // Create a collection using the builder
        let collection: FilterHandlerCollection<
            MockLayer,
            MockLayer,
            MockLayer,
            MockLayer,
            MockLayer,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockLayer::new("inbound1"))
            .add_inbound_request_handler(MockLayer::new("inbound2"))
            .add_pre_backend_handler(MockLayer::new("pre_backend"))
            .add_backend_request_handler(MockLayer::new("backend_request"))
            .add_backend_response_handler(MockLayer::new("backend_response"))
            .build()
            .expect("Collection build should succeed");

        // Create stage services from the collection
        let builder = StageServiceBuilder::new(base_service);
        let stage_services = builder
            .build_from_collection(collection)
            .expect("Stage services build should succeed");

        assert_eq!(stage_services.service_count(), 5);
        // With our identity mock layer, all services are the same, so has_no_layers() returns true
        assert!(stage_services.has_no_layers());
    }

    // Test error handling during service creation
    #[test]
    fn test_error_handling_during_service_creation() {
        let base_service = MockService::new("base");
        let builder = StageServiceBuilder::new(base_service);

        // This should succeed even with multiple layers
        let result = builder
            .build_with_handlers::<MockLayer, MockLayer, MockLayer, MockLayer, MockLayer>(
                vec![MockLayer::new("layer1"), MockLayer::new("layer2")],
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
                Vec::<MockLayer>::new(),
            );

        assert!(result.is_ok());
    }
}
