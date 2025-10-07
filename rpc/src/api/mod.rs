use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::middleware::rpc;
use jsonrpsee::types::ErrorObject;
use serde::{Deserialize, Serialize};
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize)]
#[error("RPC error")]
pub struct RpcError;

impl Into<ErrorObject<'static>> for RpcError {
    fn into(self) -> ErrorObject<'static> {
        todo!()
    }
}

#[cfg_attr(feature = "client", rpc(client))]
#[cfg_attr(feature = "server", rpc(server))]
pub trait RpcApi {
    #[method(name = "getListenerConfiguration")]
    async fn listener_configuration(&self, listener_ref: ListenerRef) -> Result<Listener, RpcError>;

    #[method(name = "getRouteConfiguration")]
    async fn route_configuration(&self, route_ref: RouteRef) -> Result<Route, RpcError>;

    #[method(name = "getBackend")]
    async fn backend(&self, name: BackendRef) -> Result<Backend, RpcError>;
}