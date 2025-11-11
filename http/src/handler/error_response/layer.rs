use crate::handler::error_response::generator::ErrorResponseGenerator;
use crate::handler::error_response::handler::ErrorResponseHandler;
use crate::handler::generator::ErrorResponseGeneratorConversionError;
use crate::stage::backend_request::BackendRequestFilter;
use crate::stage::inbound_request::InboundRequestFilterHandler;
use crate::stage::response::ResponseFilter;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::policy::error_response::ErrorResponsePolicy;

#[derive(Debug, Default, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ErrorResponseHandlerLayer {
    generator: ErrorResponseGenerator,
}

#[derive(Debug, Error)]
pub enum ErrorResponseHandlerLayerError {
    #[error(transparent)]
    Generator(#[from] ErrorResponseGeneratorConversionError),
}

impl TryFrom<&ErrorResponsePolicy> for ErrorResponseHandlerLayer {
    type Error = ErrorResponseHandlerLayerError;

    fn try_from(value: &ErrorResponsePolicy) -> Result<Self, Self::Error> {
        let generator: ErrorResponseGenerator = value.format().try_into()?;

        let layer = Self::builder().generator(generator).build();

        Ok(layer)
    }
}

impl Layer<InboundRequestFilterHandler> for ErrorResponseHandlerLayer {
    type Service = ErrorResponseHandler<InboundRequestFilterHandler>;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .generator(self.generator.clone())
            .build()
    }
}

impl Layer<BackendRequestFilter> for ErrorResponseHandlerLayer {
    type Service = ErrorResponseHandler<BackendRequestFilter>;

    fn layer(&self, inner: BackendRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .generator(self.generator.clone())
            .build()
    }
}

impl Layer<ResponseFilter> for ErrorResponseHandlerLayer {
    type Service = ErrorResponseHandler<ResponseFilter>;

    fn layer(&self, inner: ResponseFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .generator(self.generator.clone())
            .build()
    }
}
