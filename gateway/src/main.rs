mod configuration;
mod http;
mod instrumentation;

use crate::configuration::location::CurrentLocationConfigurator;
use crate::configuration::{SourceConfigurationRegistry, SourceConfigurationRegistryOptions};
use crate::http::backend::{BackendConfigurator, BackendConfiguratorOptions};
use crate::http::filter::{SharedFilterHandlersManager, SharedFilterHandlersManagerOptions};
use ::http::Uri;

use std::error::Error;
use std::sync::Arc;
use tokio::select;
use tokio::task::JoinSet;
use vg_core::instrumentation::init;
use vg_core::net::topology::TopologyLocation;
use vg_rpc_client::{ConfigurationClient, ConfigurationEventsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init("vg-gateway");

    // Validate connection parameters at startup
    let listener_ref = "example_listener".to_string();
    let server_uri = Uri::from_static("ws://localhost:9000");

    // Validate URI format
    if server_uri.scheme_str() != Some("ws") && server_uri.scheme_str() != Some("wss") {
        return Err("Invalid URI scheme: must be 'ws' or 'wss'".into());
    }

    if listener_ref.is_empty() {
        return Err("Listener reference cannot be empty".into());
    }

    // Create configuration client with proper error handling
    let client =
        match ConfigurationClient::connect_production(listener_ref.clone(), server_uri.clone())
            .await
        {
            Ok(client) => {
                tracing::info!(
                    "Successfully connected configuration client to {}",
                    server_uri
                );
                client
            }
            Err(e) => {
                tracing::error!(
                    "Failed to connect configuration client to {}: {}",
                    server_uri,
                    e
                );
                return Err(format!("Configuration client connection failed: {}", e).into());
            }
        };

    // Create events client with proper error handling
    let events = match ConfigurationEventsClient::connect_production(
        listener_ref.clone(),
        server_uri.clone(),
    )
    .await
    {
        Ok(events) => {
            tracing::info!("Successfully connected events client to {}", server_uri);
            events
        }
        Err(e) => {
            tracing::error!("Failed to connect events client to {}: {}", server_uri, e);
            return Err(format!("Events client connection failed: {}", e).into());
        }
    };

    // Validate client connectivity by attempting to fetch listener configuration
    // This is a non-blocking validation - if it fails, we log a warning but continue
    // The robust client will handle retries and reconnection automatically
    match client.listener().await {
        Ok(_) => {
            tracing::info!("Successfully validated client connectivity");
        }
        Err(e) => {
            tracing::warn!(
                "Client connectivity validation failed (this may be expected during startup): {}",
                e
            );
            // Don't fail startup here as the configuration service might not be ready yet
            // The robust client will handle retries and reconnection
        }
    }

    let events_rx = events.events();

    let current_location = CurrentLocationConfigurator::new();

    let source_configuration: SourceConfigurationRegistry =
        SourceConfigurationRegistryOptions::builder()
            .client(client)
            .events(events_rx)
            .build()
            .into();

    let shared_filter_handlers: SharedFilterHandlersManager =
        SharedFilterHandlersManagerOptions::builder()
            .routing(source_configuration.routing())
            .build()
            .into();

    let backends_configurator: BackendConfigurator = BackendConfiguratorOptions::builder()
        .current_location(current_location.current_location())
        .backends(source_configuration.backends())
        .build()
        .into();

    let current_location = current_location.start();
    let _ = current_location.send(Arc::new(
        TopologyLocation::builder()
            .zone("us-west-1".to_string())
            .node("a".to_string())
            .build(),
    ));

    let mut rrx = source_configuration.routing();
    let mut brx = backends_configurator.backends();
    let mut sfhx = shared_filter_handlers.handlers();

    let shared_filter_handlers = shared_filter_handlers.start();
    let backends_configurator = backends_configurator.start();
    let source_configuration = source_configuration.start();

    // Start event client with proper error handling
    let event_client = match events.start().await {
        Ok(handle) => {
            tracing::info!("Successfully started events client");
            handle
        }
        Err(e) => {
            tracing::error!("Failed to start events client: {}", e);
            return Err(format!("Events client startup failed: {}", e).into());
        }
    };

    let mut js = JoinSet::new();

    js.spawn(shared_filter_handlers.stopped());
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
                _ = sfhx.changed() => {
                    println!("fully resolved handlers: {:?}", sfhx.current());
                }
            }
        }
    });

    js.join_all().await;

    Ok(())
}
