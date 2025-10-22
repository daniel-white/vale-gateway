use crate::instrumentation::TRACER;
use dashmap::DashMap;
use getset::{CloneGetters, Getters};
use jsonrpsee_core::server::{
    ConnectionId, PendingSubscriptionSink, SubscriptionMessage, SubscriptionSink,
};
use opentelemetry::trace::{FutureExt, SpanKind, Tracer};
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_config::provider::ConfigurationProvider;
use vg_rpc::{ApiError, Event, EventMessage, RequestContext};

#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = "pub(crate)"), builder_type(vis = "pub(crate)"))]
pub struct PendingEventSink {
    listener_ref: ListenerRef,
    sink: PendingSubscriptionSink,
}

impl PendingEventSink {
    pub fn connection_id(&self) -> ConnectionId {
        self.sink.connection_id()
    }

    pub async fn accept(self) -> Result<EventSink, ()> {
        let sink =  self.sink.accept().await.map_err(|_| ())?; // TODO: handle error) 
        
        let sink = EventSink::builder()
            .listener_ref(self.listener_ref)
            .sink(sink)
            .build();

        let _ = sink.send(Event::Initialize).await?; // TODO handle error
        
        Ok(sink)
    }

    pub async fn reject(self, error: ApiError) {
        self.sink.reject(error).await
    }
}

#[derive(Debug, Clone, TypedBuilder, Getters)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct EventSink {
    #[getset(get = "pub")]
    listener_ref: ListenerRef,
    sink: SubscriptionSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventSinkId(ConnectionId);

impl EventSink {
    pub fn id(&self) -> EventSinkId {
        EventSinkId(self.sink.connection_id())
    }

    pub fn is_closed(&self) -> bool {
        self.sink.is_closed()
    }

    pub async fn closed(&self) {
        self.sink.closed().await
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
}

#[derive(Clone, TypedBuilder, CloneGetters)]
#[builder(builder_method(vis = "pub(crate)"), builder_type(vis = "pub(crate)"))]
pub struct EventSinkRegistry {
    #[builder(default, setter(skip))]
    #[getset(get_clone = "pub(crate)")]
    sinks: Arc<DashMap<EventSinkId, EventSink>>,
    configuration: Arc<dyn ConfigurationProvider>
}

impl EventSinkRegistry {
    pub(crate) async fn try_register(
        &self,
        pending_sink: PendingEventSink,
    ) -> Result<(), ApiError> {
        if !self.configuration.listener_exists(&pending_sink.listener_ref).await {
            pending_sink.reject(ApiError::NotFound).await;
            return Err(ApiError::NotFound)
        }

        let sink = pending_sink.accept().await.map_err(|_| ApiError::Unknown)?;
        self.sinks.insert(sink.id(), sink);

        Ok(())
    }
}
