mod instrumentation;
use crate::instrumentation::TRACER;
use async_trait::async_trait;
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::task::JoinSet;
use vg_config::http::backend::{Backend, BackendEndpoint, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::policy::ListenerPolicies;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::provider::HttpConfigurationProvider;
use vg_config::http::route::{Route, RouteRef};
use vg_core::instrumentation::init;
use vg_rpc_server::{Event, ConfigurationEventServer, ApiServerOptions};

pub struct HttpConfigProvider;

#[async_trait]
impl HttpConfigurationProvider for HttpConfigProvider {
    async fn listener(&self, listener_ref: ListenerRef) -> Option<Listener> {
        let beref = BackendRef::from("be1".to_string());
        let l = Listener::builder()
            .ref_(listener_ref)
            .policies(ListenerPolicies::default())
            .backend_refs(vec![beref])
            .route_refs(Vec::new())
            .shared_filter_refs(Vec::new())
            .filters(Vec::new())
            .build();

        Some(l)
    }

    async fn route(&self, route_ref: RouteRef) -> Option<Route> {
        None
    }

    async fn backend(&self, backend_ref: BackendRef) -> Option<Backend> {
        let ep1 = BackendEndpoint::builder()
            .addrs(Vec::new())
            .node(Some("a".to_string()))
            .zone(Some("us-west-1".to_string()))
            .build();

        let ep2 = BackendEndpoint::builder()
            .addrs(Vec::new())
            .node(Some("b".to_string()))
            .zone(Some("us-west-1".to_string()))
            .build();

        let be = Backend::builder()
            .ref_(BackendRef::from("be1".to_string()))
            .endpoints(vec![ep1, ep2])
            .build();

        Some(be)
    }

    async fn shared_filter(&self, filter_ref: SharedFilterRef) -> Option<SharedFilter> {
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init("vg-controller");

    let mut join_set: JoinSet<()> = JoinSet::new();

    let http_config = Box::from(HttpConfigProvider);
    let event_server = ConfigurationEventServer::new();
    let options = ApiServerOptions::builder()
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
            async {
                let span = TRACER.start("lc");
                let context = Context::current().with_span(span);
                sender
                    .send(
                        "example_listener".to_string().into(),
                        Event::ListenerChanged,
                    )
                    .with_context(context)
                    .await;
            }
            .await;

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            async {
                let span = TRACER.start("rc");
                let context = Context::current().with_span(span);
                sender
                    .send(
                        "example_listener".to_string().into(),
                        Event::RouteChanged(RouteRef::from("a route".to_string())),
                    )
                    .with_context(context)
                    .await;
            }
            .await;
        }
    });

    join_set.join_all().await;

    Ok(())
}
