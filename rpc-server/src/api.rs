use crate::ConfigurationEventSinkRegistry;
use crate::events::PendingConfigurationEventSink;
use async_trait::async_trait;
use jsonrpsee_core::SubscriptionResult;
use jsonrpsee_core::server::PendingSubscriptionSink;
use typed_builder::TypedBuilder;
use vg_config::http::backend::Backend;
use vg_config::http::filter::SharedFilter;
use vg_config::http::listener::Listener;
use vg_config::http::provider::HttpConfigurationProvider;
use vg_config::http::route::Route;
use vg_rpc::{ConfigurationApiError, ConfigurationApiServer, GetBackendRequest, GetListenerRequest, GetRouteRequest, GetSharedFilterRequest, SubscribeEventsRequest};

#[derive(TypedBuilder)]
pub struct ConfigurationApiServerMethods {
    event_sinks: ConfigurationEventSinkRegistry,
    http_configuration: Box<dyn HttpConfigurationProvider>,
}

#[async_trait]
impl ConfigurationApiServer for ConfigurationApiServerMethods {
    async fn listener(&self, req: GetListenerRequest) -> Result<Listener, ConfigurationApiError> {
        self.http_configuration
            .listener(req.listener_ref())
            .await
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn route(&self, req: GetRouteRequest) -> Result<Route, ConfigurationApiError> {
        self.http_configuration
            .route(req.route_ref())
            .await
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn backend(&self, req: GetBackendRequest) -> Result<Backend, ConfigurationApiError> {
        self.http_configuration
            .backend(req.backend_ref())
            .await
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn shared_filter(&self, req: GetSharedFilterRequest) -> Result<SharedFilter, ConfigurationApiError> {
        self.http_configuration
            .shared_filter(req.filter_ref())
            .await
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn events(
        &self,
        subscription_sink: PendingSubscriptionSink,
        req: SubscribeEventsRequest,
    ) -> SubscriptionResult {
        let pending_sink = PendingConfigurationEventSink::builder()
            .listener_ref(req.listener_ref())
            .sink(subscription_sink)
            .build();
        let _ = self.event_sinks.try_register(pending_sink).await;
        // TODO: handle error
        Ok(())
    }
}
