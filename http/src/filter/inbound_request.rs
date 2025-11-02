use std::future::ready;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::util::{BoxCloneService};
use tower::{Service};
use futures::future::BoxFuture;
use bytes::Bytes;
use derive_more::Constructor;

pub trait InboundRequestFilter: Service<http::request::Parts, Response = InboundRequestFilterResponse>  {}

pub enum InboundRequestFilterResponse {
    Continue,
    Respond(http::Response<Option<Bytes>>),
}

#[derive(Debug, Error)]
#[error("Inbound request filter error")]
pub struct InboundRequestFilterError;

impl<T> InboundRequestFilter for T
where
    T: Service<
        http::request::Parts,
        Response = InboundRequestFilterResponse,
        Error = InboundRequestFilterError,
    >,
{}

pub type DynInboundRequestFilter = BoxCloneService<http::request::Parts, InboundRequestFilterResponse, InboundRequestFilterError>;

#[derive(Debug, Clone, Constructor)]
pub struct InboundRequestFilterChainFinalizer;

impl Service<http::request::Parts> for InboundRequestFilterChainFinalizer {
    type Response = InboundRequestFilterResponse;
    type Error = InboundRequestFilterError;
    
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::request::Parts) -> Self::Future {
        Box::pin(ready(Ok(InboundRequestFilterResponse::Continue)))
    }
}