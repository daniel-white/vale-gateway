use crate::handler::client_addr::extractors::{ClientAddrExtractor, ClientAddrExtractorConversionError};
use crate::handler::client_addr::handler::ClientAddrFilterHandler;
use crate::stage::inbound_request::InboundRequestFilterHandler;
use http::HeaderName;
use std::sync::Arc;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::policy::client_addrs::ClientAddrPolicy;

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ClientAddrFilterHandlerLayer {
    extractor: Arc<ClientAddrExtractor>,
    backend_header: Arc<Option<HeaderName>>,
}

impl Layer<InboundRequestFilterHandler> for ClientAddrFilterHandlerLayer {
    type Service = ClientAddrFilterHandler;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .extractor(self.extractor.clone())
            .backend_header(self.backend_header.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum ClientAddrFilterHandlerLayerError {
    #[error(transparent)]
    Extractor(#[from] ClientAddrExtractorConversionError),
}

impl TryFrom<&ClientAddrPolicy> for ClientAddrFilterHandlerLayer {
    type Error = ClientAddrFilterHandlerLayerError;

    fn try_from(value: &ClientAddrPolicy) -> Result<Self, Self::Error> {
        let extractor: ClientAddrExtractor = value.extractor().try_into()?;

        let handler = Self::builder()
            .extractor(Arc::new(extractor))
            .backend_header(Arc::new(value.backend_header()))
            .build();

        Ok(handler)
    }
}
