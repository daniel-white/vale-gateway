mod events;
pub mod location;
pub mod processor;

use processor::ConfigurationProcessor;
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
use vg_core::sync::arc_watch::Receiver;
use vg_core::sync::arc_watch::Sender;
use vg_core::sync::arc_watch::channel;
use vg_core::sync::broadcast::Traced;
use vg_core::sync::handles::{handles, Handle};
use vg_rpc_client::api::ApiClient;
use vg_rpc_client::events::error::RecvError;
use vg_rpc_client::events::EventReceiver;

#[derive(Default, Debug, Getters, TypedBuilder)]
pub struct RoutingConfiguration {
    #[getset(get = "pub")]
    listener: Option<Arc<Listener>>,
    #[getset(get = "pub")]
    routes: HashMap<RouteRef, Arc<Route>>,
    #[getset(get = "pub")]
    shared_filters: HashMap<SharedFilterRef, Arc<SharedFilter>>,
}

#[derive(Default, Debug, Getters, TypedBuilder)]
pub struct BackendConfiguration {
    #[getset(get = "pub")]
    backends: HashMap<BackendRef, Arc<Backend>>,
}

#[derive(TypedBuilder)]
pub struct ConfigurationRegistryOptions {
    api_client: ApiClient,
    events: EventReceiver,
}

impl From<ConfigurationRegistryOptions> for ConfigurationRegistry {
    fn from(value: ConfigurationRegistryOptions) -> Self {
        let (backends, _) = channel();
        let (routing, _) = channel();

        Self::builder()
            .api_client(value.api_client)
            .events(value.events)
            .backends(backends)
            .routing(routing)
            .build()
    }
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ConfigurationRegistry {
    api_client: ApiClient,
    events: EventReceiver,
    backends: Sender<BackendConfiguration>,
    routing: Sender<RoutingConfiguration>,
}

impl ConfigurationRegistry {
    pub fn backends(&self) -> Receiver<BackendConfiguration> {
        self.backends.subscribe()
    }

    pub fn routing(&self) -> Receiver<RoutingConfiguration> {
        self.routing.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut events = self.events;

        let processor = ConfigurationProcessor::builder()
            .api_client(self.api_client)
            .routing(self.routing)
            .backends(self.backends)
            .build();

        spawn(async move {
            processor.init().await;
            loop {
                select! {
                    result = events.recv() => {
                        match result {
                            Ok(Traced { value: event, context }) => {
                                let span = TRACER.span_builder("SourceConfigurationRegistry::recv::ok")
                                .with_kind(SpanKind::Consumer)
                                .start_with_context(&*TRACER, &context);
                                let context = Context::current().with_span(span);
                                processor.handle(event).with_context(context).await
                            }
                            Err(RecvError::Lagged) => processor.init().await,
                            Err(RecvError::Closed) => break,
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
