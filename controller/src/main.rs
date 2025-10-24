mod instrumentation;
use crate::instrumentation::TRACER;
use async_trait::async_trait;
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use http::StatusCode;
use tokio::task::JoinSet;
use vg_config::http::backend::{Backend, BackendEndpoint, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::filter::static_response::{StaticResponseFilter, StaticResponseFilterRef, StaticResponseSharedFilter};
use vg_config::http::listener::policy::ListenerPolicies;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::provider::ConfigurationProvider;
use vg_config::http::route::{Route, RouteRef};
use vg_config::http::route::host::HostMatcher;
use vg_core::instrumentation::init;
use vg_rpc_server::api::{ApiServer, ApiServerOptions};
use vg_rpc_server::events::{Event, EventBroker, EventBrokerOptions};

pub struct HttpConfigProvider;

#[async_trait]
impl ConfigurationProvider for HttpConfigProvider {
    async fn listener(&self, listener_ref: &ListenerRef) -> Option<Listener> {
        let beref = BackendRef::from("be1".to_string());
        let r = RouteRef::from("r".to_string());
        let f = StaticResponseFilterRef::from("f".to_string());
        let f = SharedFilterRef::StaticResponse(f);
        let l = Listener::builder()
            .ref_(listener_ref.clone())
            .policies(ListenerPolicies::default())
            .backend_refs(vec![beref])
            .route_refs(vec![r])
            .shared_filter_refs(vec![f])
            .filters(Vec::new())
            .build();

        Some(l)
    }
    

    async fn route(&self, route_ref: &RouteRef) -> Option<Route> {
        let r = Route::builder()
            .ref_(route_ref.clone())
            .host_matchers(vec![HostMatcher::Exact("example.com.".to_string())])
            .rules(Vec::new())
            .build();
        
        Some(r)
    }

    async fn backend(&self, backend_ref: &BackendRef) -> Option<Backend> {
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

    async fn shared_filter(&self, filter_ref: &SharedFilterRef) -> Option<SharedFilter> {
        let  f = StaticResponseFilter::builder()
            .status_code(StatusCode::ACCEPTED)
            .body(None).build();
        let f = StaticResponseSharedFilter::builder()
            .ref_(StaticResponseFilterRef::from("f".to_string()))
            .filter(f)
            .build();
        let f =    
        SharedFilter::StaticResponse(f);
        Some(f)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init("vg-controller");

    let mut join_set: JoinSet<()> = JoinSet::new();
    let configuration = Arc::from(HttpConfigProvider);
    let event_broker: EventBroker = EventBrokerOptions::builder()
        .configuration(configuration.clone())
        .capacity(1024)
        .build()
        .into();
    
    let api_server: ApiServer = ApiServerOptions::builder()
        .binding(SocketAddr::from_str("0.0.0.0:9000").unwrap())
        .event_sinks(event_broker.sinks())
        .configuration(configuration)
        .build()
        .into();

    let event_sender = event_broker.sender();

    let api_server = api_server.start().await?;
    let event_broker = event_broker.start();

    join_set.spawn(api_server.stopped());
    join_set.spawn(event_broker.stopped());

    join_set.spawn(async move {
        loop {
            async {
                let span = TRACER.start("lc");
                let context = Context::current().with_span(span);
                event_sender
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
                event_sender
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
