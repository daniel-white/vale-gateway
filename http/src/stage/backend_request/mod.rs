use crate::handler::error_code::ErrorResponseCode;
use crate::stage::inbound_request::InboundRequestFilterHandler;
use bytes::Bytes;
use derive_more::{Deref, DerefMut, From};
use thiserror::Error;
use tower::Service;
use tower::util::BoxCloneService;

pub mod factory;
mod finalizer;

trait BackendRequestFilterTrait: Service<http::request::Parts> {}

#[derive(Debug, From)]
pub enum BackendRequestFilterResult {
    Continue(http::request::Parts),
    ErrorResponse(ErrorResponseCode),
    Respond(http::Response<Option<Bytes>>),
}

#[derive(Debug, Error)]
#[error("Backend request filter error")]
pub struct BackendRequestFilterError;

impl<T> BackendRequestFilterTrait for T where
    T: Service<
            http::request::Parts,
            Response = BackendRequestFilterResult,
            Error = BackendRequestFilterError,
        >
{
}

pub type BackendRequestFilter =
    BoxCloneService<http::request::Parts, BackendRequestFilterResult, BackendRequestFilterError>;

#[derive(Debug, Deref, DerefMut, From)]
pub struct BackendRequestFilterChain(BackendRequestFilter);
