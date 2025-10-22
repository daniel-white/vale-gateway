use std::sync::Arc;
use async_trait::async_trait;
use jsonrpsee_core::server::PendingSubscriptionSink;
use jsonrpsee_core::SubscriptionResult;
use typed_builder::TypedBuilder;
use vg_config::http::backend::Backend;
use vg_config::http::filter::SharedFilter;
use vg_config::http::listener::Listener;
use vg_config::provider::DataProvider;
use vg_config::http::route::Route;
use vg_rpc::{ApiError, ApiServer, GetBackendRequest, GetListenerRequest, GetRouteRequest, GetSharedFilterRequest, SubscribeEventsRequest};
use crate::events::sinks::{EventSinkRegistry, PendingEventSink};

#[derive(TypedBuilder)]
pub struct ApiServerMethods {
    event_sinks: EventSinkRegistry,
    data_provider: Arc<dyn DataProvider>,
}

#[async_trait]
impl ApiServer for ApiServerMethods {
    async fn listener(&self, req: GetListenerRequest) -> Result<Listener, ApiError> {
        self.data_provider
            .listener(req.listener_ref())
            .await
            .ok_or(ApiError::NotFound)
    }

    async fn route(&self, req: GetRouteRequest) -> Result<Route, ApiError> {
        self.data_provider
            .route(req.route_ref())
            .await
            .ok_or(ApiError::NotFound)
    }

    async fn backend(&self, req: GetBackendRequest) -> Result<Backend, ApiError> {
        self.data_provider
            .backend(req.backend_ref())
            .await
            .ok_or(ApiError::NotFound)
    }

    async fn shared_filter(&self, req: GetSharedFilterRequest) -> Result<SharedFilter, ApiError> {
        self.data_provider
            .shared_filter(req.filter_ref())
            .await
            .ok_or(ApiError::NotFound)
    }

    async fn events(
        &self,
        subscription_sink: PendingSubscriptionSink,
        req: SubscribeEventsRequest,
    ) -> SubscriptionResult {
        let pending_sink = PendingEventSink::builder()
            .listener_ref(req.listener_ref())
            .sink(subscription_sink)
            .build();
        let _ = self.event_sinks.try_register(pending_sink).await;
        // TODO: handle error
        Ok(())
    }
}