pub mod extensions;
pub mod extractors;

use std::sync::Arc;
use crate::filter::handlers::client_addr::extensions::{RequestSocketAddr, TrustedClientIpAddr};
use crate::filter::handlers::client_addr::extractors::{
    ClientAddrExtractor, ClientAddrExtractorConversionError,
};
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use thiserror::Error;
use tower::{Layer, Service};
use typed_builder::TypedBuilder;
use vg_config::http::policy::client_addrs::ClientAddrPolicy;
use crate::filter::inbound_request::{DynInboundRequestFilter, InboundRequestFilterError, InboundRequestFilterResponse};

#[derive(Debug, Clone, TypedBuilder)]
pub struct ClientAddrFilterHandler {
    inner: DynInboundRequestFilter,
    extractor: Arc<ClientAddrExtractor>,
    backend_header: Arc<Option<HeaderName>>,
}

impl Service<Parts> for ClientAddrFilterHandler {
    type Response = InboundRequestFilterResponse;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Parts) -> Self::Future {
        let addr = req
            .extensions
            .get::<RequestSocketAddr>()
            .expect("request socket addr not set")
            .addr();

        if let Some(header) = self.backend_header.as_ref() {
            req.headers.remove(header);
        }

        if let Some(ip_addr) = self.extractor.extract(addr, &req) {
            let extension = TrustedClientIpAddr::builder().ip_addr(ip_addr).build();

            req.extensions.insert(extension);
            if let Some(header) = self.backend_header.as_ref() {
                let header_value = HeaderValue::from_str(&ip_addr.to_string())
                    .expect("Failed to convert IP to HeaderValue");
                req.headers.insert(header, header_value);
            }
        }

        self.inner.call(req)
    }
}

#[derive(Debug, Clone, TypedBuilder)]
pub struct ClientAddrFilterHandlerLayer {
    #[builder(setter(into))]
    extractor: Arc<ClientAddrExtractor>,
    backend_header: Arc<Option<HeaderName>>,
}


impl Layer<DynInboundRequestFilter> for ClientAddrFilterHandlerLayer {
    type Service = ClientAddrFilterHandler;

    fn layer(&self, inner: DynInboundRequestFilter) -> Self::Service {
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
            .extractor(extractor)
            .backend_header(Arc::new(value.backend_header()))
            .build();

        Ok(handler)
    }
}
