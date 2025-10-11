mod configuration;

use std::error::Error;
//use crate::configuration::{ConfigurationRegistry, ConfigurationRegistrySynchronizer};
use async_from::AsyncTryInto;
use http::Uri;
use tokio::task::JoinSet;
use vg_rpc_client::{
    ConfigurationClient, ConfigurationEventClient, ConfigurationTransport,
    ConfigurationTransportOptions,
};
use crate::configuration::SourceConfigurationRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let transport_options = ConfigurationTransportOptions::builder()
        .address(Uri::from_static("ws://localhost:9000"))
        .listener_ref("example_listener".to_string())
        .build();
    let transport: ConfigurationTransport = transport_options.async_try_into().await?;

    let client = ConfigurationClient::builder()
        .transport(transport.clone())
        .build();

    let event_client = ConfigurationEventClient::new(transport);

    let event_rx = event_client.receiver();

    let source_configuration = SourceConfigurationRegistry::builder()
        .client(client)
        .event_rx(event_rx)
        .build();

    let source_configuration = source_configuration.start();
    let event_client = event_client.start().await?;


    let mut js = JoinSet::new();

    js.spawn(event_client.stopped());
    js.spawn(source_configuration.stopped());

    js.join_all().await;

    Ok(())
}
