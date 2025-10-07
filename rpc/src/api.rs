use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObject;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};

#[derive(Debug, Error, Serialize, Deserialize)]
#[error("RPC error")]
pub enum RpcApiError {
    NotFound,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ListenerConfigurationEvent {
    Changed,
    RouteChanged(RouteRef),
    RouteRemoved(RouteRef),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BackendEvent {
    Changed(BackendRef),
    Removed(BackendRef),
}

impl From<RpcApiError> for ErrorObject<'static> {
    fn from(val: RpcApiError) -> Self {
        match val {
            RpcApiError::NotFound => ErrorObject::owned(1, "Not Found", None::<()>),
        }
    }
}

impl From<ErrorObject<'static>> for RpcApiError {
    fn from(val: ErrorObject<'static>) -> Self {
        match val.code() {
            1 => RpcApiError::NotFound,
            _ => RpcApiError::NotFound,
        }
    }
}

#[cfg_attr(feature = "client", rpc(client))]
#[cfg_attr(feature = "server", rpc(server))]
pub trait RpcApi {
    #[subscription(name = "subscribe_listener_configuration", item = ListenerConfigurationEvent)]
    async fn watch_listener_configuration(&self, listener_ref: ListenerRef);

    #[method(name = "get_listener_configuration")]
    async fn listener_configuration(&self, listener_ref: ListenerRef)
    -> Result<Listener, RpcApiError>;

    #[method(name = "get_route_configuration")]
    async fn route_configuration(&self, route_ref: RouteRef) -> Result<Route, RpcApiError>;

    #[subscription(name = "subscribe_backends", item = BackendEvent)]
    async fn watch_backends(&self, listener_ref: ListenerRef);

    #[method(name = "get_backend")]
    async fn backend(&self, name: BackendRef) -> Result<Backend, RpcApiError>;
}
