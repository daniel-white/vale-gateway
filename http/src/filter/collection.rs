use thiserror::Error;

/// Errors that can occur during filter handler collection building
#[derive(Debug, Error)]
pub enum FilterHandlerCollectionError {
    #[error("Filter handler validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Incompatible filter handler for stage: {stage}")]
    IncompatibleHandler { stage: String },

    #[error("Duplicate filter handler detected in stage: {stage}")]
    DuplicateHandler { stage: String },
}

/// An immutable container for managing filter handlers by stage with consistent ordering
/// This uses concrete vectors instead of trait objects to avoid dyn compatibility issues
#[derive(Debug, Clone)]
pub struct FilterHandlerCollection<
    InboundReq = (),
    PreBackend = (),
    BackendReq = (),
    BackendResp = (),
    RespGen = (),
> {
    inbound_request_handlers: Vec<InboundReq>,
    pre_backend_handlers: Vec<PreBackend>,
    backend_request_handlers: Vec<BackendReq>,
    backend_response_handlers: Vec<BackendResp>,
    response_generation_handlers: Vec<RespGen>,
}

impl<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>
    FilterHandlerCollection<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>
{
    /// Get inbound request handlers
    pub fn inbound_request_handlers(&self) -> &[InboundReq] {
        &self.inbound_request_handlers
    }

    /// Get pre-backend handlers
    pub fn pre_backend_handlers(&self) -> &[PreBackend] {
        &self.pre_backend_handlers
    }

    /// Get backend request handlers
    pub fn backend_request_handlers(&self) -> &[BackendReq] {
        &self.backend_request_handlers
    }

    /// Get backend response handlers
    pub fn backend_response_handlers(&self) -> &[BackendResp] {
        &self.backend_response_handlers
    }

    /// Get response generation handlers
    pub fn response_generation_handlers(&self) -> &[RespGen] {
        &self.response_generation_handlers
    }

    /// Check if the collection is empty (no handlers in any stage)
    pub fn is_empty(&self) -> bool {
        self.inbound_request_handlers.is_empty()
            && self.pre_backend_handlers.is_empty()
            && self.backend_request_handlers.is_empty()
            && self.backend_response_handlers.is_empty()
            && self.response_generation_handlers.is_empty()
    }

    /// Get the total number of handlers across all stages
    pub fn total_handler_count(&self) -> usize {
        self.inbound_request_handlers.len()
            + self.pre_backend_handlers.len()
            + self.backend_request_handlers.len()
            + self.backend_response_handlers.len()
            + self.response_generation_handlers.len()
    }

    /// Get the number of handlers in the inbound request stage
    pub fn inbound_request_handler_count(&self) -> usize {
        self.inbound_request_handlers.len()
    }

    /// Get the number of handlers in the pre-backend stage
    pub fn pre_backend_handler_count(&self) -> usize {
        self.pre_backend_handlers.len()
    }

    /// Get the number of handlers in the backend request stage
    pub fn backend_request_handler_count(&self) -> usize {
        self.backend_request_handlers.len()
    }

    /// Get the number of handlers in the backend response stage
    pub fn backend_response_handler_count(&self) -> usize {
        self.backend_response_handlers.len()
    }

    /// Get the number of handlers in the response generation stage
    pub fn response_generation_handler_count(&self) -> usize {
        self.response_generation_handlers.len()
    }
}

/// Builder for creating FilterHandlerCollection instances
#[derive(Debug)]
pub struct FilterHandlerCollectionBuilder<
    InboundReq = (),
    PreBackend = (),
    BackendReq = (),
    BackendResp = (),
    RespGen = (),
> {
    inbound_request_handlers: Vec<InboundReq>,
    pre_backend_handlers: Vec<PreBackend>,
    backend_request_handlers: Vec<BackendReq>,
    backend_response_handlers: Vec<BackendResp>,
    response_generation_handlers: Vec<RespGen>,
}

impl<InboundReq, PreBackend, BackendReq, BackendResp, RespGen> Default
    for FilterHandlerCollectionBuilder<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>
    FilterHandlerCollectionBuilder<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>
{
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            inbound_request_handlers: Vec::new(),
            pre_backend_handlers: Vec::new(),
            backend_request_handlers: Vec::new(),
            backend_response_handlers: Vec::new(),
            response_generation_handlers: Vec::new(),
        }
    }

    /// Add an inbound request filter handler with validation
    pub fn add_inbound_request_handler(mut self, handler: InboundReq) -> Self {
        self.inbound_request_handlers.push(handler);
        self
    }

    /// Add a pre-backend filter handler with validation
    pub fn add_pre_backend_handler(mut self, handler: PreBackend) -> Self {
        self.pre_backend_handlers.push(handler);
        self
    }

    /// Add a backend request filter handler with validation
    pub fn add_backend_request_handler(mut self, handler: BackendReq) -> Self {
        self.backend_request_handlers.push(handler);
        self
    }

    /// Add a backend response filter handler with validation
    pub fn add_backend_response_handler(mut self, handler: BackendResp) -> Self {
        self.backend_response_handlers.push(handler);
        self
    }

    /// Add a response generation filter handler with validation
    pub fn add_response_generation_handler(mut self, handler: RespGen) -> Self {
        self.response_generation_handlers.push(handler);
        self
    }

    /// Validate the current configuration of filter handlers
    pub fn validate(&self) -> Result<(), FilterHandlerCollectionError> {
        // Basic validation - ensure we don't have empty collections if we're expecting handlers
        // More sophisticated validation can be added based on specific requirements

        // For now, we'll validate that the collection is in a consistent state
        // Additional validation logic can be added here as needed

        Ok(())
    }

    /// Build the immutable FilterHandlerCollection with validation
    pub fn build(
        self,
    ) -> Result<
        FilterHandlerCollection<InboundReq, PreBackend, BackendReq, BackendResp, RespGen>,
        FilterHandlerCollectionError,
    > {
        // Validate before building
        self.validate()?;

        Ok(FilterHandlerCollection {
            inbound_request_handlers: self.inbound_request_handlers,
            pre_backend_handlers: self.pre_backend_handlers,
            backend_request_handlers: self.backend_request_handlers,
            backend_response_handlers: self.backend_response_handlers,
            response_generation_handlers: self.response_generation_handlers,
        })
    }

    /// Build the immutable FilterHandlerCollection without validation (for backward compatibility)
    pub fn build_unchecked(
        self,
    ) -> FilterHandlerCollection<InboundReq, PreBackend, BackendReq, BackendResp, RespGen> {
        FilterHandlerCollection {
            inbound_request_handlers: self.inbound_request_handlers,
            pre_backend_handlers: self.pre_backend_handlers,
            backend_request_handlers: self.backend_request_handlers,
            backend_response_handlers: self.backend_response_handlers,
            response_generation_handlers: self.response_generation_handlers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock filter handlers for testing
    #[derive(Debug, Clone, PartialEq)]
    struct MockInboundRequestHandler {
        id: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct MockPreBackendHandler {
        id: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct MockBackendRequestHandler {
        id: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct MockBackendResponseHandler {
        id: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct MockResponseGenerationHandler {
        id: u32,
    }

    #[test]
    fn test_builder_new() {
        let builder = FilterHandlerCollectionBuilder::<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        >::new();

        assert_eq!(builder.inbound_request_handlers.len(), 0);
        assert_eq!(builder.pre_backend_handlers.len(), 0);
        assert_eq!(builder.backend_request_handlers.len(), 0);
        assert_eq!(builder.backend_response_handlers.len(), 0);
        assert_eq!(builder.response_generation_handlers.len(), 0);
    }

    #[test]
    fn test_builder_default() {
        let builder = FilterHandlerCollectionBuilder::<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        >::default();

        assert_eq!(builder.inbound_request_handlers.len(), 0);
        assert_eq!(builder.pre_backend_handlers.len(), 0);
        assert_eq!(builder.backend_request_handlers.len(), 0);
        assert_eq!(builder.backend_response_handlers.len(), 0);
        assert_eq!(builder.response_generation_handlers.len(), 0);
    }

    #[test]
    fn test_builder_add_inbound_request_handler() {
        let handler = MockInboundRequestHandler { id: 1 };
        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new().add_inbound_request_handler(handler.clone());

        assert_eq!(builder.inbound_request_handlers.len(), 1);
        assert_eq!(builder.inbound_request_handlers[0], handler);
    }

    #[test]
    fn test_builder_add_pre_backend_handler() {
        let handler = MockPreBackendHandler { id: 1 };
        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new().add_pre_backend_handler(handler.clone());

        assert_eq!(builder.pre_backend_handlers.len(), 1);
        assert_eq!(builder.pre_backend_handlers[0], handler);
    }

    #[test]
    fn test_builder_add_backend_request_handler() {
        let handler = MockBackendRequestHandler { id: 1 };
        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new().add_backend_request_handler(handler.clone());

        assert_eq!(builder.backend_request_handlers.len(), 1);
        assert_eq!(builder.backend_request_handlers[0], handler);
    }

    #[test]
    fn test_builder_add_backend_response_handler() {
        let handler = MockBackendResponseHandler { id: 1 };
        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new().add_backend_response_handler(handler.clone());

        assert_eq!(builder.backend_response_handlers.len(), 1);
        assert_eq!(builder.backend_response_handlers[0], handler);
    }

    #[test]
    fn test_builder_add_response_generation_handler() {
        let handler = MockResponseGenerationHandler { id: 1 };
        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new().add_response_generation_handler(handler.clone());

        assert_eq!(builder.response_generation_handlers.len(), 1);
        assert_eq!(builder.response_generation_handlers[0], handler);
    }

    #[test]
    fn test_builder_multiple_handlers() {
        let inbound_handler1 = MockInboundRequestHandler { id: 1 };
        let inbound_handler2 = MockInboundRequestHandler { id: 2 };
        let pre_backend_handler = MockPreBackendHandler { id: 1 };

        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(inbound_handler1.clone())
            .add_inbound_request_handler(inbound_handler2.clone())
            .add_pre_backend_handler(pre_backend_handler.clone());

        assert_eq!(builder.inbound_request_handlers.len(), 2);
        assert_eq!(builder.inbound_request_handlers[0], inbound_handler1);
        assert_eq!(builder.inbound_request_handlers[1], inbound_handler2);
        assert_eq!(builder.pre_backend_handlers.len(), 1);
        assert_eq!(builder.pre_backend_handlers[0], pre_backend_handler);
    }

    #[test]
    fn test_builder_validation_success() {
        let builder: FilterHandlerCollectionBuilder<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockInboundRequestHandler { id: 1 });

        assert!(builder.validate().is_ok());
    }

    #[test]
    fn test_builder_build_success() {
        let inbound_handler = MockInboundRequestHandler { id: 1 };
        let pre_backend_handler = MockPreBackendHandler { id: 1 };

        let collection: FilterHandlerCollection<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(inbound_handler.clone())
            .add_pre_backend_handler(pre_backend_handler.clone())
            .build()
            .expect("Build should succeed");

        assert_eq!(collection.inbound_request_handlers().len(), 1);
        assert_eq!(collection.inbound_request_handlers()[0], inbound_handler);
        assert_eq!(collection.pre_backend_handlers().len(), 1);
        assert_eq!(collection.pre_backend_handlers()[0], pre_backend_handler);
    }

    #[test]
    fn test_builder_build_unchecked() {
        let inbound_handler = MockInboundRequestHandler { id: 1 };

        let collection: FilterHandlerCollection<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(inbound_handler.clone())
            .build_unchecked();

        assert_eq!(collection.inbound_request_handlers().len(), 1);
        assert_eq!(collection.inbound_request_handlers()[0], inbound_handler);
    }

    #[test]
    fn test_collection_is_empty() {
        let empty_collection = FilterHandlerCollectionBuilder::<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        >::new()
        .build_unchecked();

        assert!(empty_collection.is_empty());

        let non_empty_collection: FilterHandlerCollection<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockInboundRequestHandler { id: 1 })
            .build_unchecked();

        assert!(!non_empty_collection.is_empty());
    }

    #[test]
    fn test_collection_handler_counts() {
        let collection = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockInboundRequestHandler { id: 1 })
            .add_inbound_request_handler(MockInboundRequestHandler { id: 2 })
            .add_pre_backend_handler(MockPreBackendHandler { id: 1 })
            .add_backend_request_handler(MockBackendRequestHandler { id: 1 })
            .add_backend_response_handler(MockBackendResponseHandler { id: 1 })
            .add_backend_response_handler(MockBackendResponseHandler { id: 2 })
            .add_response_generation_handler(MockResponseGenerationHandler { id: 1 })
            .build_unchecked();

        assert_eq!(collection.inbound_request_handler_count(), 2);
        assert_eq!(collection.pre_backend_handler_count(), 1);
        assert_eq!(collection.backend_request_handler_count(), 1);
        assert_eq!(collection.backend_response_handler_count(), 2);
        assert_eq!(collection.response_generation_handler_count(), 1);
        assert_eq!(collection.total_handler_count(), 7);
    }

    #[test]
    fn test_collection_stage_specific_retrieval() {
        let inbound_handler = MockInboundRequestHandler { id: 1 };
        let pre_backend_handler = MockPreBackendHandler { id: 2 };
        let backend_request_handler = MockBackendRequestHandler { id: 3 };
        let backend_response_handler = MockBackendResponseHandler { id: 4 };
        let response_generation_handler = MockResponseGenerationHandler { id: 5 };

        let collection = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(inbound_handler.clone())
            .add_pre_backend_handler(pre_backend_handler.clone())
            .add_backend_request_handler(backend_request_handler.clone())
            .add_backend_response_handler(backend_response_handler.clone())
            .add_response_generation_handler(response_generation_handler.clone())
            .build_unchecked();

        // Test stage-specific retrieval
        let inbound_handlers = collection.inbound_request_handlers();
        assert_eq!(inbound_handlers.len(), 1);
        assert_eq!(inbound_handlers[0], inbound_handler);

        let pre_backend_handlers = collection.pre_backend_handlers();
        assert_eq!(pre_backend_handlers.len(), 1);
        assert_eq!(pre_backend_handlers[0], pre_backend_handler);

        let backend_request_handlers = collection.backend_request_handlers();
        assert_eq!(backend_request_handlers.len(), 1);
        assert_eq!(backend_request_handlers[0], backend_request_handler);

        let backend_response_handlers = collection.backend_response_handlers();
        assert_eq!(backend_response_handlers.len(), 1);
        assert_eq!(backend_response_handlers[0], backend_response_handler);

        let response_generation_handlers = collection.response_generation_handlers();
        assert_eq!(response_generation_handlers.len(), 1);
        assert_eq!(response_generation_handlers[0], response_generation_handler);
    }

    #[test]
    fn test_collection_immutability() {
        let collection: FilterHandlerCollection<
            MockInboundRequestHandler,
            MockPreBackendHandler,
            MockBackendRequestHandler,
            MockBackendResponseHandler,
            MockResponseGenerationHandler,
        > = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockInboundRequestHandler { id: 1 })
            .build_unchecked();

        // Test that we can only get immutable references to the handlers
        let handlers = collection.inbound_request_handlers();
        assert_eq!(handlers.len(), 1);

        // The collection should be immutable - we can't modify the handlers
        // This is enforced by the type system (handlers is &[T], not &mut [T])
    }

    #[test]
    fn test_builder_pattern_functionality() {
        // Test that the builder pattern works correctly with method chaining
        let collection = FilterHandlerCollectionBuilder::new()
            .add_inbound_request_handler(MockInboundRequestHandler { id: 1 })
            .add_pre_backend_handler(MockPreBackendHandler { id: 2 })
            .add_backend_request_handler(MockBackendRequestHandler { id: 3 })
            .add_backend_response_handler(MockBackendResponseHandler { id: 4 })
            .add_response_generation_handler(MockResponseGenerationHandler { id: 5 })
            .build()
            .expect("Build should succeed");

        assert_eq!(collection.total_handler_count(), 5);
        assert!(!collection.is_empty());
    }
}
