pub mod sinks;

pub use vg_rpc::Event;

use crate::events::sinks::EventSinkRegistry;
use crate::instrumentation::TRACER;
use getset::CloneGetters;
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, SpanKind, TraceContextExt, Tracer};
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_config::provider::ConfigurationProvider;
use vg_core::sync::handles::{Handle, handles};
use vg_core::sync::mpsc::{Receiver, Sender, Traced, channel};

#[derive(TypedBuilder)]
pub struct EventBrokerOptions {
    capacity: usize,
    configuration: Arc<dyn ConfigurationProvider>,
}

#[derive(TypedBuilder, CloneGetters)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct EventBroker {
    #[getset(get_clone = "pub")]
    sinks: EventSinkRegistry,
    sender: Sender<(ListenerRef, Event)>,
    receiver: Receiver<(ListenerRef, Event)>,
}

impl From<EventBrokerOptions> for EventBroker {
    fn from(value: EventBrokerOptions) -> Self {
        let (sender, receiver) = channel(value.capacity);
        let sinks = EventSinkRegistry::builder()
            .configuration(value.configuration)
            .build();
        Self::builder()
            .sinks(sinks)
            .sender(sender)
            .receiver(receiver)
            .build()
    }
}

impl EventBroker {
    pub fn sender(&self) -> EventSender {
        EventSender::builder().sender(self.sender.clone()).build()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            let sinks = self.sinks.sinks();
            let mut receiver = self.receiver;
            loop {
                select! {
                    value = receiver.recv() => {
                        match value {
                            Some(Traced { value: (listener_ref, event), context }) => {
                                let span = TRACER.start("ConfigurationEventServer::recv");
                                let context = context.with_span(span);
                                let current_sinks: Vec<_> = sinks.iter()
                                    .filter(|entry| &listener_ref == entry.listener_ref() && !entry.is_closed())
                                    .collect();

                                for sink in current_sinks {
                                    let _ = sink.send(event.clone())
                                        .with_context(context.clone()).await;
                                    // TODO handle error
                                }

                                sinks.retain(|_, sink| !sink.is_closed());
                            },
                            None => break
                        }
                    }
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }
        });

        handle
    }
}

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct EventSender {
    sender: Sender<(ListenerRef, Event)>,
}

impl EventSender {
    pub async fn send(&self, listener_ref: ListenerRef, event: Event) {
        let span = TRACER
            .span_builder("ConfigurationEventSender::send")
            .with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        if let Err(err) = self
            .sender
            .send((listener_ref, event))
            .with_context(context)
            .await
        {
            println!("Error sending: {:?}", err)
        }
    }
}
