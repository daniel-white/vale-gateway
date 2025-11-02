use std::future::ready;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::util::{BoxCloneService};
use tower::{Service};
use futures::future::BoxFuture;
use derive_more::Constructor;
use crate::filter::inbound_request::InboundRequestFilterResponse;

pub trait BackendRequestFilter: Service<http::request::Parts>  {}

#[derive(Debug, Error)]
#[error("Backend request filter error")]
pub struct BackendRequestFilterError;

impl<T> BackendRequestFilter for T
where
    T: Service<
        http::request::Parts,
        Response = (),
        Error = BackendRequestFilterError,
    >,
{}

pub type DynBackendRequestFilter = BoxCloneService<http::request::Parts, (), BackendRequestFilterError>;

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