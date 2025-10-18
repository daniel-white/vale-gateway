mod configuration;
mod http;
mod instrumentation;

use crate::configuration::{SourceConfigurationRegistry, SourceConfigurationRegistryOptions};
use ::http::Uri;
use async_from::AsyncTryInto;
use std::error::Error;
use std::sync::Arc;
use tokio::select;
use tokio::task::JoinSet;
use vg_core::instrumentation::init;
use vg_core::net::topology::TopologyLocation;
use vg_rpc_client::{
    ConfigurationClient, ConfigurationEventClient, ConfigurationTransport,
    ConfigurationTransportOptions,
};
use crate::configuration::location::CurrentLocationConfigurator;
use crate::http::backend::{BackendConfigurator, BackendConfiguratorOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init("vg-gateway");

    let transport: ConfigurationTransport = ConfigurationTransportOptions::builder()
        .address(Uri::from_static("ws://localhost:9000"))
        .listener_ref("example_listener".to_string())
        .build()
        .async_try_into()
        .await?;

    let client = ConfigurationClient::builder()
        .transport(transport.clone())
        .build();

    let event_client = ConfigurationEventClient::new(transport);

    let event_rx = event_client.receiver();
    
    let current_location = CurrentLocationConfigurator::new();

    let source_configuration: SourceConfigurationRegistry =
        SourceConfigurationRegistryOptions::builder()
            .client(client)
            .event_rx(event_rx)
            .build()
            .into();
    
    let backends_configurator: BackendConfigurator = BackendConfiguratorOptions::builder()
        .current_location_rx(current_location.current_location())
        .source_backends_rx(source_configuration.backends())
        .build()
        .into();
    
    let current_location = current_location.start();
    let vg_core = current_location.send(Arc::new(TopologyLocation::builder().zone("us-west-1".to_string()).node("a".to_string()).build()));

    let mut rrx = source_configuration.routing();
    let mut brx = backends_configurator.backends();

    let backends_configurator = backends_configurator.start();
    let source_configuration = source_configuration.start();
    let event_client = event_client.start().await?;

    let mut js = JoinSet::new();

    js.spawn(backends_configurator.stopped());
    js.spawn(event_client.stopped());
    js.spawn(source_configuration.stopped());

    js.spawn(async move {
        loop {
            select! {
                _ = rrx.changed() => {
                    println!("routing: {:?}", rrx.current());
                }
                _ = brx.changed() => {
                    println!("fully resolved backend: {:?}", brx.current());
                }
            }
        }
    });

    js.join_all().await;

    Ok(())
}
