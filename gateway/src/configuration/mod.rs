mod events;
pub mod location;

use crate::configuration::events::processor::ConfigurationEventProcessor;
use crate::instrumentation::TRACER;
use getset::Getters;
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, SpanKind, TraceContextExt, Tracer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
pub use vg_core::sync::arc_watch::Receiver;
use vg_core::sync::arc_watch::Sender;
use vg_core::sync::arc_watch::channel;
use vg_core::sync::broadcast::Traced;
use vg_core::sync::handles::{Handle, handles};
use vg_rpc_client::{
    Client,
};
use vg_rpc_client::events::error::RecvError;
use vg_rpc_client::events::EventReceiver;

#[derive(Default, Debug, Clone, Getters, TypedBuilder)]
pub struct SourceRoutingConfiguration {
    #[getset(get = "pub")]
    listener: Option<Arc<Listener>>,
    #[getset(get = "pub")]
    routes: HashMap<Arc<RouteRef>, Arc<Route>>,
    #[getset(get = "pub")]
    shared_filters: HashMap<Arc<SharedFilterRef>, Arc<SharedFilter>>,
}

#[derive(Default, Debug, Clone, Getters, TypedBuilder)]
pub struct SourceBackendConfiguration {
    #[getset(get = "pub")]
    backends: HashMap<Arc<BackendRef>, Arc<Backend>>,
}

#[derive(TypedBuilder)]
pub struct SourceConfigurationRegistryOptions {
    client: Client,
    events: EventReceiver,
}

impl From<SourceConfigurationRegistryOptions> for SourceConfigurationRegistry {
    fn from(value: SourceConfigurationRegistryOptions) -> Self {
        let (backends_tx, _) = channel();
        let (routing_tx, _) = channel();

        Self::builder()
            .client(value.client)
            .events(value.events)
            .backends_tx(backends_tx)
            .routing_tx(routing_tx)
            .build()
    }
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct SourceConfigurationRegistry {
    client: Client,
    events: EventReceiver,
    backends_tx: Sender<SourceBackendConfiguration>,
    routing_tx: Sender<SourceRoutingConfiguration>,
}

impl SourceConfigurationRegistry {
    pub fn backends(&self) -> Receiver<SourceBackendConfiguration> {
        self.backends_tx.subscribe()
    }

    pub fn routing(&self) -> Receiver<SourceRoutingConfiguration> {
        self.routing_tx.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut events = self.events;

        let processor = ConfigurationEventProcessor::builder()
            .client(self.client)
            .routing_tx(self.routing_tx)
            .backends_tx(self.backends_tx)
            .build();

        spawn(async move {
            processor.init().await;
            loop {
                select! {
                    value = events.recv() => {
                        match value {
                            Ok(Traced { value: event, context }) => {
                                let span = TRACER.span_builder("SourceConfigurationRegistry::recv::ok")
                                .with_kind(SpanKind::Consumer)
                                .start_with_context(&*TRACER, &context);
                                let context = Context::current().with_span(span);
                                processor.handle(event).with_context(context).await;
                            }
                            Err(RecvError::Lagged) => {
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
