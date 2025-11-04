use crate::filter::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterResult,
};
use derive_more::Constructor;
use futures::future::BoxFuture;
use std::future::ready;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Debug, Clone, Constructor)]
pub struct InboundRequestFilterChainFinalizer;

impl Service<http::request::Parts> for InboundRequestFilterChainFinalizer {
    type Response = InboundRequestFilterResult;
    type Error = InboundRequestFilterError;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: http::request::Parts) -> Self::Future {
        Box::pin(ready(Ok(InboundRequestFilterResult::Continue)))
    }
}
