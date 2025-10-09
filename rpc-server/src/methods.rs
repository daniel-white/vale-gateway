use crate::events::{ConfigurationEventManager, PendingConfigurationEventSink};
use async_trait::async_trait;
use http::HeaderName;
use jsonrpsee_core::SubscriptionResult;
use jsonrpsee_core::server::PendingSubscriptionSink;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::policy::ListenerPolicies;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::policy::client_addrs::ClientAddressesPolicy;
use vg_config::http::policy::error_response::ErrorResponsePolicy;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{ConfigurationApiError, ConfigurationApiServer};

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationApiServerMethods {
    event_manager: ConfigurationEventManager,
}

#[async_trait]
impl ConfigurationApiServer for ConfigurationApiServerMethods {
    async fn listener(&self, listener_ref: ListenerRef) -> Result<Listener, ConfigurationApiError> {
        println!("Fetching listener configuration for: {:?}", listener_ref);
        let cap = ClientAddressesPolicy::builder()
            .backend_header(HeaderName::from_static("x-real-ip"))
            .build();
        let listener_policies = ListenerPolicies::builder()
            .client_addresses(cap)
            .error_response(ErrorResponsePolicy::default())
            .build();
        let listener = Listener::builder()
            .ref_(listener_ref.clone())
            .policies(listener_policies)
            .build();
        Ok(listener)
    }

    async fn route(&self, route_ref: RouteRef) -> Result<Route, ConfigurationApiError> {
        todo!()
    }

    async fn backend(&self, backend_ref: BackendRef) -> Result<Backend, ConfigurationApiError> {
        todo!()
    }

    async fn watch_events(
        &self,
        subscription_sink: PendingSubscriptionSink,
        listener_ref: ListenerRef,
    ) -> SubscriptionResult {
        let pending_sink = PendingConfigurationEventSink::builder()
            .listener_ref(listener_ref.clone())
            .sink(subscription_sink)
            .build();
        let _ = self.event_manager.try_subscribe(pending_sink).await;
        // TODO: handle error
        Ok(())
    }
}
