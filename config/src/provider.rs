use crate::http::backend::{Backend, BackendRef};
use crate::http::filter::{SharedFilter, SharedFilterRef};
use crate::http::gateway::{Gateway, GatewayRef};
use crate::http::route::{Route, RouteRef};
use async_trait::async_trait;

#[async_trait]
pub trait ConfigurationProvider: Send + Sync {
    async fn gateway(&self, gateway_ref: &GatewayRef) -> Option<Gateway>;

    async fn gateway_exists(&self, gateway_ref: &GatewayRef) -> bool {
        self.gateway(gateway_ref).await.is_some()
    }

    async fn route(&self, route_ref: &RouteRef) -> Option<Route>;

    async fn backend(&self, backend_ref: &BackendRef) -> Option<Backend>;

    async fn shared_filter(&self, filter_ref: &SharedFilterRef) -> Option<SharedFilter>;
}
