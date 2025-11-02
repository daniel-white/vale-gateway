use std::future::ready;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::util::{BoxCloneService};
use tower::{Service};
use futures::future::BoxFuture;
use derive_more::Constructor;

pub trait ResponseFilter: Service<http::response::Parts>  {}

#[derive(Debug, Error)]
#[error("Backend request filter error")]
pub struct ResponseFilterError;

impl<T> ResponseFilter for T
where
    T: Service<
        http::response::Parts,
        Response = (),
        Error = ResponseFilterError,
    >,
{}

pub type DynResponseFilter = BoxCloneService<http::response::Parts, (), ResponseFilterError>;

#[derive(Debug, Clone, Constructor)]
pub struct ResponseFilterChainFinalizer;

impl Service<http::response::Parts> for ResponseFilterChainFinalizer {
    type Response = ();
    type Error = ResponseFilterError;
    
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::response::Parts) -> Self::Future {
        Box::pin(ready(Ok(())))
    }
}