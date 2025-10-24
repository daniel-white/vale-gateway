use crate::http::backend::{Backend, BackendRef};
use crate::http::filter::{SharedFilter, SharedFilterRef};
use crate::http::listener::{Listener, ListenerRef};
use crate::http::route::{Route, RouteRef};
use async_trait::async_trait;

#[async_trait]
pub trait ConfigurationProvider: Send + Sync {
    async fn listener(&self, listener_ref: &ListenerRef) -> Option<Listener>;

    async fn listener_exists(&self, listener_ref: &ListenerRef) -> bool {
        self.listener(listener_ref).await.is_some()
    }

    async fn route(&self, route_ref: &RouteRef) -> Option<Route>;

    async fn backend(&self, backend_ref: &BackendRef) -> Option<Backend>;

    async fn shared_filter(&self, filter_ref: &SharedFilterRef) -> Option<SharedFilter>;
}
