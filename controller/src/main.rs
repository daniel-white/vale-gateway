mod instrumentation;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::str::FromStr;
use opentelemetry::trace::{FutureExt, Span, Tracer};
use tokio::task::JoinSet;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::policy::ListenerPolicies;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::provider::HttpConfigurationProvider;
use vg_config::http::route::{Route, RouteRef};
use vg_core::instrumentation::init;
use vg_rpc_server::{ConfigurationEvent, ConfigurationEventServer, ConfigurationServerOptions};
use crate::instrumentation::TRACER;

pub struct HttpConfigProvider;

#[async_trait]
impl HttpConfigurationProvider for HttpConfigProvider {
    async fn listener(&self, listener_ref: ListenerRef) -> Option<Listener> {
        let l = Listener::builder()
            .ref_(listener_ref)
            .policies(ListenerPolicies::default())
            .backend_refs(Vec::new())
            .route_refs(Vec::new())
            .build();

        Some(l)
    }

    async fn route(&self, route_ref: RouteRef) -> Option<Route> {
        None
    }

    async fn backend(&self, backend_ref: BackendRef) -> Option<Backend> {
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init();

    let mut join_set: JoinSet<()> = JoinSet::new();

    let http_config = Box::from(HttpConfigProvider);
    let event_server = ConfigurationEventServer::new();
    let options = ConfigurationServerOptions::builder()
        .binding(SocketAddr::from_str("0.0.0.0:9000").unwrap())
        .event_sinks(event_server.sinks())
        .http_configuration(http_config)
        .build();

    let sender = event_server.sender();

    let server = options.start_server().await?;
    let event_server = event_server.start();

    join_set.spawn(server.stopped());
    join_set.spawn(event_server.stopped());

    join_set.spawn(async move {
        loop {
            let mut span = TRACER.start("lc");
            sender
                .send(
                    "example_listener".to_string().into(),
                    ConfigurationEvent::ListenerChanged,
                )
                .with_current_context()
                .await;
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            span.end();

            let mut span = TRACER.start("rc");
            sender
                .send(
                    "example_listener".to_string().into(),
                    ConfigurationEvent::RouteChanged(RouteRef::from("a route".to_string())),
                )
                .with_current_context()
                .await;
            span.end();
        }
    });

    join_set.join_all().await;

    Ok(())
}
