use crate::stage::response::{ResponseFilterError, ResponseFilterResult};
use derive_more::Constructor;
use futures::future::BoxFuture;
use std::future::ready;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Debug, Clone, Constructor)]
pub struct ResponseFilterChainFinalizer;

impl Service<http::response::Parts> for ResponseFilterChainFinalizer {
    type Response = ResponseFilterResult;
    type Error = ResponseFilterError;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, res: http::response::Parts) -> Self::Future {
        Box::pin(ready(Ok(res.into())))
    }
}
