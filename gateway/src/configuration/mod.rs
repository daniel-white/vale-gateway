mod events;
pub mod location;
pub mod processor;

use crate::instrumentation::TRACER;
use getset::Getters;
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, SpanKind, TraceContextExt, Tracer};
use processor::ConfigurationProcessor;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::gateway::Gateway;
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::broadcast::Traced;
use vg_core::sync::handles::{Handle, handles};
use vg_core::sync::observable::{Observable, Subscription};
use vg_rpc_client::api::ApiClient;
use vg_rpc_client::events::EventReceiver;
use vg_rpc_client::events::error::RecvError;

#[derive(Default, Debug, Getters, TypedBuilder, PartialEq)]
pub struct GatewayConfiguration {
    #[getset(get = "pub")]
    gateway: Option<Arc<Gateway>>,
    #[getset(get = "pub")]
    routes: HashMap<RouteRef, Arc<Route>>,
    #[getset(get = "pub")]
    shared_filters: HashMap<SharedFilterRef, Arc<SharedFilter>>,
}

#[derive(Default, Debug, Getters, TypedBuilder, PartialEq)]
pub struct BackendConfiguration {
    #[getset(get = "pub")]
    backends: HashMap<BackendRef, Arc<Backend>>,
}

#[derive(TypedBuilder)]
pub struct ConfigurationRegistryOptions {
    api_client: ApiClient,
    events: EventReceiver,
    gateway_ref: vg_config::http::gateway::GatewayRef,
}

impl From<ConfigurationRegistryOptions> for ConfigurationRegistry {
    fn from(value: ConfigurationRegistryOptions) -> Self {
        Self::builder()
            .api_client(value.api_client)
            .events(value.events)
            .gateway_ref(value.gateway_ref)
            .build()
    }
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ConfigurationRegistry {
    api_client: ApiClient,
    events: EventReceiver,
    gateway_ref: vg_config::http::gateway::GatewayRef,
    #[builder(default, setter(skip))]
    backends: Observable<BackendConfiguration>,
    #[builder(default, setter(skip))]
    gateway: Observable<GatewayConfiguration>,
}

impl ConfigurationRegistry {
    pub fn backends(&self) -> Subscription<BackendConfiguration> {
        self.backends.subscribe()
    }

    pub fn gateway(&self) -> Subscription<GatewayConfiguration> {
        self.gateway.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut events = self.events;

        let processor = ConfigurationProcessor::builder()
            .api_client(self.api_client)
            .gateway(self.gateway)
            .backends(self.backends)
            .gateway_ref(self.gateway_ref)
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
