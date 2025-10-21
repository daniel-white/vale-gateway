use crate::instrumentation::TRACER;
use dashmap::DashMap;
use getset::Getters;
use jsonrpsee_core::server::{
    ConnectionId, PendingSubscriptionSink, SubscriptionMessage, SubscriptionSink,
};
use opentelemetry::trace::{FutureExt, SpanKind, Tracer};
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_rpc::{
    ApiError, Event, EventMessage, RequestContext,
};

#[derive(Debug, TypedBuilder)]
pub struct PendingConfigurationEventSink {
    listener_ref: ListenerRef,
    sink: PendingSubscriptionSink,
}

impl PendingConfigurationEventSink {
    pub fn connection_id(&self) -> ConnectionId {
        self.sink.connection_id()
    }

    pub async fn accept(self) -> Result<ConfigurationEventSink, ()> {
        match self.sink.accept().await {
            Ok(sink) => Ok(ConfigurationEventSink::builder()
                .listener_ref(self.listener_ref)
                .sink(sink)
                .build()),
            Err(_) => Err(()), // TODO: handle error
        }
    }

    pub async fn reject(self, error: ApiError) -> Result<(), ApiError> {
        self.sink.reject(error).await;
        Ok(())
    }
}

#[derive(Debug, Clone, TypedBuilder, Getters)]
pub struct ConfigurationEventSink {
    #[getset(get = "pub")]
    listener_ref: ListenerRef,
    sink: SubscriptionSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConfigurationEventSinkId(ConnectionId);

impl ConfigurationEventSink {
    pub fn connection_id(&self) -> ConfigurationEventSinkId {
        ConfigurationEventSinkId(self.sink.connection_id())
    }

    pub fn is_closed(&self) -> bool {
        self.sink.is_closed()
    }

    pub async fn send(&self, event: Event) -> Result<(), ()> {
        // TODO handle serialization error
        let span = TRACER
            .span_builder("ConfigurationEventSink::send")
            .with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let message = EventMessage::builder()
            .context(RequestContext::new(span))
            .event(event)
            .build();

        let message = SubscriptionMessage::new(
            self.sink.method_name(),
            self.sink.subscription_id(),
            &message,
        )
        .unwrap();
        self.sink
            .send(message)
            .with_current_context()
            .await
            .map_err(|_| ())
    }

    pub async fn closed(&self) {
        self.sink.closed().await
    }
}

#[derive(Debug, TypedBuilder)]
pub struct EventSinkRegistry {
    sinks: Arc<DashMap<ConfigurationEventSinkId, ConfigurationEventSink>>,
}

impl EventSinkRegistry {
    pub async fn try_register(
        &self,
        pending_sink: PendingConfigurationEventSink,
    ) -> Result<(), ApiError> {
        // TODO validate and accept/reject the pending sink

        let sink = pending_sink.accept().await.unwrap();
        self.sinks.insert(sink.connection_id(), sink);

        Ok(())
    }
}
