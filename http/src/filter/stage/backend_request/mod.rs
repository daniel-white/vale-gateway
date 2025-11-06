use derive_more::{Deref, DerefMut, From};
use thiserror::Error;
use tower::Service;
use tower::util::BoxCloneService;
use crate::filter::stage::inbound_request::InboundRequestFilterHandler;

pub mod factory;
mod finalizer;

trait BackendRequestFilterTrait: Service<http::request::Parts> {}

#[derive(Debug, Error)]
#[error("Backend request filter error")]
pub struct BackendRequestFilterError;

impl<T> BackendRequestFilterTrait for T where
    T: Service<http::request::Parts, Response = (), Error = BackendRequestFilterError>
{
}

pub type BackendRequestFilter =
    BoxCloneService<http::request::Parts, (), BackendRequestFilterError>;

#[derive(Debug, Deref, DerefMut, From)]
pub struct BackendRequestFilterChain(BackendRequestFilter);

