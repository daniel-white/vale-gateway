use http::HeaderName;
use jsonrpsee_core::async_trait;
use jsonrpsee_core::server::PendingSubscriptionSink;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::listener::policy::ListenerPolicies;
use vg_config::http::policy::client_addrs::ClientAddressesPolicy;
use vg_config::http::policy::client_addrs::ClientAddressExtractor::TrustedHeader;
use vg_config::http::policy::error_response::ErrorResponsePolicy;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::api::{RpcApiError, RpcApiServer};

#[derive(Debug, Default)]
pub struct RpcApiServerImpl;

#[async_trait]
impl RpcApiServer for RpcApiServerImpl {
    async fn listener_configuration(&self, listener_ref: ListenerRef) -> Result<Listener, RpcApiError> {
        return Err(RpcApiError::NotFound);
        println!("Fetching listener configuration for: {:?}", listener_ref);
        let cap = ClientAddressesPolicy::builder().backend_header(HeaderName::from_static("x-real-ip")).build();
        let listener_policies = ListenerPolicies::builder()
            .client_addresses(cap)
            .error_response(ErrorResponsePolicy::default())
            .build();
        let listener = Listener::builder()
            .ref_(listener_ref)
            .policies(listener_policies).build();
        Ok(listener)
    }

    async fn route_configuration(&self, route_ref: RouteRef) -> Result<Route, RpcApiError> {
        todo!()
    }

    async fn backend(&self, name: BackendRef) -> Result<Backend, RpcApiError> {
        todo!()
    }

    async fn watch_listener_configuration(&self, subscription_sink: PendingSubscriptionSink, listener_ref: ListenerRef) {
        todo!()
    }

    async fn watch_backends(&self, subscription_sink: PendingSubscriptionSink, listener_ref: ListenerRef) {
        todo!()
    }
}