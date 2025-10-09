use crate::events::{ConfigurationEventManager, PendingConfigurationEventSink};
use async_trait::async_trait;
use jsonrpsee_core::SubscriptionResult;
use jsonrpsee_core::server::PendingSubscriptionSink;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::provider::HttpConfigurationProvider;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{ConfigurationApiError, ConfigurationApiServer};

#[derive(TypedBuilder)]
pub struct ConfigurationApiServerMethods {
    event_manager: ConfigurationEventManager,
    http_configuration: Box<dyn HttpConfigurationProvider>,
}

#[async_trait]
impl ConfigurationApiServer for ConfigurationApiServerMethods {
    async fn listener(&self, listener_ref: ListenerRef) -> Result<Listener, ConfigurationApiError> {
        self.http_configuration
            .listener(listener_ref)
            .await
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn route(&self, route_ref: RouteRef) -> Result<Route, ConfigurationApiError> {
        self.http_configuration
            .route(route_ref)
            .await
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn backend(&self, backend_ref: BackendRef) -> Result<Backend, ConfigurationApiError> {
        self.http_configuration
            .backend(backend_ref)
            .await
            .ok_or(ConfigurationApiError::NotFound)
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
