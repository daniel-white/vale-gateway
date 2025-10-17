mod events;

use crate::configuration::events::processor::ConfigurationEventProcessor;
use crate::configuration::events::watch::SourceConfigurationSender;
pub use events::watch::SourceConfigurationWatch;
use events::watch::channel;
use getset::Getters;
use opentelemetry::trace::{FutureExt, SpanKind, TraceContextExt, Tracer};
use std::collections::HashMap;
use std::sync::Arc;
use opentelemetry::Context;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::broadcast::{Traced, WithContext};
use vg_core::sync::handles::{Handle, handles};
use vg_rpc_client::{ConfigurationClient, ConfigurationEventReceiver, ConfigurationEventRecvError};
use crate::instrumentation::TRACER;

#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct SourceRoutingConfiguration {
    #[getset(get = "pub")]
    listener: Listener,
    #[getset(get = "pub")]
    routes: HashMap<RouteRef, Route>,
}

#[derive(Default, Debug, Clone, Getters, TypedBuilder)]
pub struct SourceBackendConfiguration {
    #[getset(get = "pub")]
    backends: HashMap<BackendRef, Backend>,
}

#[derive(TypedBuilder)]
pub struct SourceConfigurationRegistryOptions {
    client: ConfigurationClient,
    event_rx: ConfigurationEventReceiver,
}

impl From<SourceConfigurationRegistryOptions> for SourceConfigurationRegistry {
    fn from(value: SourceConfigurationRegistryOptions) -> Self {
        let (backends_tx, backends_rx) = channel();
        let (routing_tx, routing_rx) = channel();

        Self::builder()
            .client(value.client)
            .event_rx(value.event_rx)
            .backends_rx(backends_rx)
            .backends_tx(backends_tx)
            .routing_rx(routing_rx)
            .routing_tx(routing_tx)
            .build()
    }
}

#[derive(TypedBuilder)]
pub struct SourceConfigurationRegistry {
    client: ConfigurationClient,
    event_rx: ConfigurationEventReceiver,
    backends_tx: SourceConfigurationSender<Arc<SourceBackendConfiguration>>,
    backends_rx: SourceConfigurationWatch<Arc<SourceBackendConfiguration>>,
    routing_tx: SourceConfigurationSender<Arc<SourceRoutingConfiguration>>,
    routing_rx: SourceConfigurationWatch<Arc<SourceRoutingConfiguration>>,
}

impl SourceConfigurationRegistry {
    pub fn backends(&self) -> SourceConfigurationWatch<Arc<SourceBackendConfiguration>> {
        self.backends_rx.clone()
    }

    pub fn routing(&self) -> SourceConfigurationWatch<Arc<SourceRoutingConfiguration>> {
        self.routing_rx.clone()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut event_rx = self.event_rx;

        let processor = ConfigurationEventProcessor::builder()
            .client(self.client)
            .routing_tx(self.routing_tx)
            .backends_tx(self.backends_tx)
            .build();

        spawn(async move {
            processor.init().await;
            loop {
                select! {
                    value = event_rx.recv() => {
                        match value {
                            Ok(Traced { value: event, context }) => {
                                let span = TRACER.span_builder("SourceConfigurationRegistry::recv::ok")
                                .with_kind(SpanKind::Consumer)
                                .start_with_context(&*TRACER, &context);
                                let context = Context::current().with_remote_span_context(context);
                                processor.handle(event).with_context(context).await;
                            }
                            Err(ConfigurationEventRecvError::Lagged) => {
                                processor.init().await;
                            }
                            _ => {
                                continue;
                            }
                        }
                    },
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }
        });

        handle
    }
}
