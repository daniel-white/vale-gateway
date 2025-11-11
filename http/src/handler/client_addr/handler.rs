use crate::extensions::{RequestSocketAddr, TrustedClientIpAddr};
use crate::handler::client_addr::extractors::ClientAddrExtractor;
use crate::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult,
};
use futures::future::BoxFuture;
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ClientAddrFilterHandler {
    inner: InboundRequestFilterHandler,
    extractor: Arc<ClientAddrExtractor>,
    backend_header: Arc<Option<HeaderName>>,
}

impl Service<Parts> for ClientAddrFilterHandler {
    type Response = InboundRequestFilterResult;
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
