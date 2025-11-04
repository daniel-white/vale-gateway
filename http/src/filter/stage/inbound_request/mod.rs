use crate::filter::handler::{
    AccessControlFilterHandlerLayer, ClientAddrFilterHandlerLayer, HeaderModifierFilterHandlerLayer,
};
use crate::policy::error_response::error_codes::ErrorResponseCode;
use bytes::Bytes;
use derive_more::From;
use thiserror::Error;
use tower::Service;
use tower::util::BoxCloneService;
use typed_builder::TypedBuilder;

pub mod factory;
mod finalizer;

trait InboundRequestFilterService:
    Service<http::request::Parts, Response = InboundRequestFilterResult>
{
}

#[derive(Debug, From)]
pub enum InboundRequestFilterResult {
    Continue,
    Reject(ErrorResponseCode),
    Respond(http::Response<Option<Bytes>>),
}

#[derive(Debug, Error)]
#[error("Inbound request filter error")]
pub struct InboundRequestFilterError;

impl<T> InboundRequestFilterService for T where
    T: Service<
            http::request::Parts,
            Response = InboundRequestFilterResult,
            Error = InboundRequestFilterError,
        >
{
}

pub type InboundRequestFilterHandler =
    BoxCloneService<http::request::Parts, InboundRequestFilterResult, InboundRequestFilterError>;

pub type InboundRequestFilterChain = InboundRequestFilterHandler;

#[derive(Debug, TypedBuilder)]
pub struct EarlyInboundRequestFilterChainFactory {
    client_addr: ClientAddrFilterHandlerLayer,
    #[builder(default)]
    access_control: Option<AccessControlFilterHandlerLayer>,
    #[builder(default)]
    header_modifiers: Vec<HeaderModifierFilterHandlerLayer>,
}
