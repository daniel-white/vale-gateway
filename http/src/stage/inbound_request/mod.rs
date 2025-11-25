use crate::handler::error_code::ErrorResponseCode;
use bytes::Bytes;
use derive_more::{Deref, DerefMut, From};
use thiserror::Error;
use tower::Service;
use tower::util::BoxCloneService;

pub use crate::stage::pre_routing_request::factory::*;

trait InboundRequestFilterTrait: Service<http::request::Parts, Response = InboundRequestFilterResult> {}

#[derive(Debug, From)]
pub enum InboundRequestFilterResult {
    Continue(http::request::Parts),
    ErrorResponse(ErrorResponseCode),
    Respond(http::Response<Option<Bytes>>),
}

#[derive(Debug, Error)]
#[error("Inbound request filter error")]
pub struct InboundRequestFilterError;

impl<T> InboundRequestFilterTrait for T where
    T: Service<http::request::Parts, Response = InboundRequestFilterResult, Error = InboundRequestFilterError>
{
}

pub type InboundRequestFilterHandler =
    BoxCloneService<http::request::Parts, InboundRequestFilterResult, InboundRequestFilterError>;

#[derive(Debug, Deref, DerefMut, From)]
pub struct InboundRequestFilterChain(InboundRequestFilterHandler);
