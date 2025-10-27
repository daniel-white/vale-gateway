mod configuration;
mod http;
mod instrumentation;
mod server;

use crate::configuration::location::CurrentLocationConfigurator;
use crate::configuration::{ConfigurationRegistry, ConfigurationRegistryOptions};
use crate::http::backend::{BackendConfigurator, BackendConfiguratorOptions};
use crate::http::filter::{SharedFilterHandlersManager, SharedFilterHandlersManagerOptions};
use crate::http::routing::route::{RouteConfigurator, RouteConfiguratorOptions};
use ::http::Uri;
use std::error::Error;
use std::sync::Arc;
use tokio::select;
use tokio::task::JoinSet;
use vg_core::instrumentation::init;
use vg_core::net::topology::TopologyLocation;
use vg_rpc_client::api::{ApiClient, ApiClientOptions};
use vg_rpc_client::events::{EventClient, EventClientOptions};
use vg_rpc_client::transport::{Transport, TransportOptions};
use crate::server::{Server, ServerOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init("vg-gateway");

    let transport: Transport = TransportOptions::builder()
        .endpoint(Uri::from_static("ws://localhost:9000"))
        .listener_ref("example_listener".to_string())
        .build()
        .try_into()?;

    let api_client: ApiClient = ApiClientOptions::builder()
        .transport_client(transport.client())
        .build()
        .into();

    let event_client: EventClient = EventClientOptions::builder()
        .capacity(32)
        .transport_client(transport.client())
        .build()
        .try_into()?;

    let current_location = CurrentLocationConfigurator::new();

    let configuration: ConfigurationRegistry = ConfigurationRegistryOptions::builder()
        .api_client(api_client)
        .events(event_client.events())
        .build()
        .into();

    let shared_filter_handlers: SharedFilterHandlersManager =
        SharedFilterHandlersManagerOptions::builder()
            .routing(configuration.routing())
            .build()
            .into();

    let backends_configurator: BackendConfigurator = BackendConfiguratorOptions::builder()
        .current_location(current_location.subscribe())
        .backends(configuration.backends())
        .build()
        .into();

    let routes_configurator: RouteConfigurator = RouteConfiguratorOptions::builder()
        .routing(configuration.routing())
        .shared_filter_handlers(shared_filter_handlers.handlers())
        .build()
        .into();
    
    let server: Server = ServerOptions::builder().services(Vec::new()).build().into();
    

    
    let mut rrx = configuration.routing();
    let mut brx = backends_configurator.backends();
    let mut sfhx = shared_filter_handlers.handlers();
    let mut routes_rx = routes_configurator.routes();

    let current_location = current_location.start();
    let transport = transport.start();
    let shared_filter_handlers = shared_filter_handlers.start();
    let backends_configurator = backends_configurator.start();
    let routes_configurator = routes_configurator.start();
    let source_configuration = configuration.start();
    let event_client = event_client.start();
    let server = server.start().unwrap();

    let mut js = JoinSet::new();

    js.spawn(current_location.stopped());
    js.spawn(transport.stopped());
    js.spawn(shared_filter_handlers.stopped());
    js.spawn(backends_configurator.stopped());
    js.spawn(routes_configurator.stopped());
    js.spawn(event_client.stopped());
    js.spawn(source_configuration.stopped());
    js.spawn(server.stopped());

    js.spawn(async move {
        loop {
            select! {
                Ok(_) = routes_rx.changed() => {
                    println!("routes: {:?}", routes_rx.current());
                },
                routing = rrx.changed() => {
                    println!("routing: {:?}", routing);
                }
                Ok(_) = brx.changed() => {
                    println!("fully resolved backend: {:?}", brx.current());
                }
                Ok(_) = sfhx.changed() => {
                    println!("fully resolved handlers: {:?}", sfhx.current());
                }
            }
        }
    });

    js.join_all().await;

    Ok(())
}
