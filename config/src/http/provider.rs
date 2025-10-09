use crate::http::backend::{Backend, BackendRef};
use crate::http::listener::{Listener, ListenerRef};
use crate::http::route::{Route, RouteRef};
use async_trait::async_trait;

#[async_trait]
pub trait HttpConfigurationProvider: Send + Sync {
    async fn listener(&self, listener_ref: ListenerRef) -> Option<Listener>;

    async fn route(&self, route_ref: RouteRef) -> Option<Route>;

    async fn backend(&self, backend_ref: BackendRef) -> Option<Backend>;
}
