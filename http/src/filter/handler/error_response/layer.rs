use std::sync::Arc;
use tower::Layer;
use typed_builder::TypedBuilder;
use crate::filter::handler::error_response::handler::ErrorResponseFilterHandler;
use crate::filter::stage::backend_request::BackendRequestFilter;
use crate::filter::stage::inbound_request::InboundRequestFilterHandler;
use crate::filter::stage::response::ResponseFilter;
use crate::policy::error_response::generators::ErrorResponseGenerator;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ErrorResponseHandlerLayer {
    generator: Arc<ErrorResponseGenerator>,
}

impl Layer<InboundRequestFilterHandler> for ErrorResponseHandlerLayer {
    type Service = ErrorResponseFilterHandler<InboundRequestFilterHandler>;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .generator(self.generator.clone())
            .build()
    }
}

impl Layer<BackendRequestFilter> for ErrorResponseHandlerLayer {
    type Service = ErrorResponseFilterHandler<BackendRequestFilter>;

    fn layer(&self, inner: BackendRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .generator(self.generator.clone())
            .build()
    }
}

impl Layer<ResponseFilter> for ErrorResponseHandlerLayer {
    type Service = ErrorResponseFilterHandler<ResponseFilter>;

    fn layer(&self, inner: ResponseFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .generator(self.generator.clone())
            .build()
    }
}