use crate::ConfigurationEventSinkRegistry;
use crate::events::sinks::{ConfigurationEventSink, ConfigurationEventSinkId};
use crate::instrumentation::TRACER;
use dashmap::DashMap;
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, SpanKind, TraceContextExt, Tracer};
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_core::sync::handles::{Handle, handles};
use vg_core::sync::mpsc::{Receiver, Sender, Traced, channel};
use vg_rpc::ConfigurationEvent;

#[derive(Debug)]
pub struct ConfigurationEventServer {
    sinks: Arc<DashMap<ConfigurationEventSinkId, ConfigurationEventSink>>,
    tx: Sender<(ListenerRef, ConfigurationEvent)>,
    rx: Receiver<(ListenerRef, ConfigurationEvent)>,
}

impl Default for ConfigurationEventServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationEventServer {
    pub fn new() -> Self {
        let (tx, rx) = channel(32);
        Self {
            sinks: Arc::new(DashMap::new()),
            tx,
            rx,
        }
    }

    pub fn sinks(&self) -> ConfigurationEventSinkRegistry {
        ConfigurationEventSinkRegistry::builder()
            .sinks(self.sinks.clone())
            .build()
    }

    pub fn sender(&self) -> ConfigurationEventSender {
        ConfigurationEventSender::builder()
            .tx(self.tx.clone())
            .build()
    }

    pub fn start(self) -> Handle {
        let (handle, stop_handle) = handles();
        let rx = self.rx;
        let sinks = self.sinks;

        spawn(async move {
            let mut rx = rx;
            let mut stop_handle = stop_handle;
            loop {
                select! {
                    value = rx.recv() => {
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
pub struct ConfigurationEventSender {
    tx: Sender<(ListenerRef, ConfigurationEvent)>,
}

impl ConfigurationEventSender {
    pub async fn send(&self, listener_ref: ListenerRef, event: ConfigurationEvent) {
        let span = TRACER
            .span_builder("ConfigurationEventSender::send")
            .with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        if let Err(err) = self
            .tx
            .send((listener_ref, event))
            .with_context(context)
            .await
        {
            println!("Error sending: {:?}", err)
        }
    }
}
