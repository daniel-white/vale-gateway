mod configuration;
mod http;
mod instrumentation;

use crate::configuration::{SourceConfigurationRegistry, SourceConfigurationRegistryOptions};
use ::http::Uri;
use async_from::AsyncTryInto;
use std::error::Error;
use tokio::select;
use tokio::task::JoinSet;
use vg_core::instrumentation::init;
use vg_rpc_client::{
    ConfigurationClient, ConfigurationEventClient, ConfigurationTransport,
    ConfigurationTransportOptions,
};

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

    let source_configuration: SourceConfigurationRegistry =
        SourceConfigurationRegistryOptions::builder()
            .client(client)
            .event_rx(event_rx)
            .build()
            .into();

    let mut rrx = source_configuration.routing();
    let mut brx = source_configuration.backends();

    let source_configuration = source_configuration.start();
    let event_client = event_client.start().await?;

    let mut js = JoinSet::new();

    js.spawn(event_client.stopped());
    js.spawn(source_configuration.stopped());

    js.spawn(async move {
        loop {
            select! {
                _ = rrx.changed() => {
                    println!("routing: {:?}", rrx.current());
                }
                _ = brx.changed() => {
                    println!("backends: {:?}", rrx.current());
                }
            }
        }
    });

    js.join_all().await;

    Ok(())
}
