use crate::filter::stage::backend_request::BackendRequestFilterError;
use derive_more::Constructor;
use futures::future::BoxFuture;
use std::future::ready;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Debug, Clone, Constructor)]
pub struct BackendRequestFilterChainFinalizer;

impl Service<http::request::Parts> for BackendRequestFilterChainFinalizer {
    type Response = ();
    type Error = BackendRequestFilterError;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::request::Parts) -> Self::Future {
        Box::pin(ready(Ok(())))
    }
}
