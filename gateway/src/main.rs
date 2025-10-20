//! # Simplified Gateway Architecture
//!
//! This gateway implementation follows a simplified architecture where:
//!
//! ## Self-Managing Clients
//! - Configuration clients handle all transport concerns internally
//! - No external connection monitoring or health checking required
//! - Automatic reconnection and error handling built-in
//! - Graceful startup mode prevents failures due to service unavailability
//!
//! ## Focused Main Function
//! - Under 100 lines as required by specification
//! - Focuses solely on component wiring and coordination
//! - No complex timeout handling or retry logic
//! - No custom connection status monitoring
//!
//! ## Component Architecture
//! - Clean separation of concerns
//! - Helper functions for specific responsibilities
//! - Minimal error handling (only critical startup errors)
//! - All resilience concerns delegated to clients

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
use tokio::task::JoinSet;
use vg_core::instrumentation::init;
use vg_core::net::topology::TopologyLocation;
use vg_rpc_client::{ConfigurationClient, ConfigurationEventsClient};

struct GatewayComponents {
    source_configuration: SourceConfigurationRegistry,
    shared_filter_handlers: SharedFilterHandlersManager,
    backends_configurator: BackendConfigurator,
    current_location: CurrentLocationConfigurator,
}

struct GatewayHandles {
    tasks: JoinSet<()>,
}

/// Gateway main entry point
///
/// This simplified main function focuses solely on initialization and error handling.
/// All complex logic has been moved to helper functions and self-managing clients.
#[tokio::main]
async fn main() {
    init("vg-gateway");

    if let Err(e) = run_gateway().await {
        tracing::error!("Gateway startup failed: {}", e);
        std::process::exit(1);
    }
}

/// Validate basic startup parameters
///
/// This function performs minimal validation of required parameters.
/// Complex validation is handled by the self-managing clients.
fn validate_startup_parameters(listener_ref: &str, server_uri: &Uri) -> Result<(), Box<dyn Error>> {
    if listener_ref.is_empty() {
        return Err("Listener reference cannot be empty".into());
    }

    match server_uri.scheme_str() {
        Some("ws") | Some("wss") => Ok(()),
        _ => Err("Invalid URI scheme: must be 'ws' or 'wss'".into()),
    }
}

/// Create and wire gateway components
///
/// This function handles the creation and wiring of gateway components.
/// All transport concerns are handled by the self-managing clients.
async fn create_gateway_components(
    client: ConfigurationClient,
    events: ConfigurationEventsClient,
) -> Result<(GatewayComponents, ConfigurationEventsClient), Box<dyn Error>> {
    let events_rx = events.events();

    let source_configuration: SourceConfigurationRegistry =
        SourceConfigurationRegistryOptions::builder()
            .client(client)
            .events(events_rx)
            .build()
            .into();

    let current_location = CurrentLocationConfigurator::new();

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

    Ok((
        GatewayComponents {
            source_configuration,
            shared_filter_handlers,
            backends_configurator,
            current_location,
        },
        events,
    ))
}

/// Start all gateway components
///
/// This function starts all gateway components and returns handles for monitoring.
/// No custom connection monitoring or health checking is performed.
async fn start_gateway_components(
    components: GatewayComponents,
    events: ConfigurationEventsClient,
) -> Result<GatewayHandles, Box<dyn Error>> {
    let mut tasks = JoinSet::new();

    // Set initial location
    let current_location_tx = components.current_location.start();
    current_location_tx.send(Arc::new(
        TopologyLocation::builder()
            .zone("us-west-1".to_string())
            .node("a".to_string())
            .build(),
    ))?;

    // Start all components
    let shared_filter_handlers_handle = components.shared_filter_handlers.start();
    let backends_configurator_handle = components.backends_configurator.start();
    let source_configuration_handle = components.source_configuration.start();

    // Start event client
    let event_client = events.start().await?;

    // Spawn component tasks
    tasks.spawn(shared_filter_handlers_handle.stopped());
    tasks.spawn(backends_configurator_handle.stopped());
    tasks.spawn(source_configuration_handle.stopped());
    tasks.spawn(event_client.stopped());

    Ok(GatewayHandles { tasks })
}

/// Wait for component completion
///
/// This function waits for any component to stop and logs the event.
/// No complex recovery or heartbeat logic is implemented.
async fn wait_for_completion(mut handles: GatewayHandles) {
    while let Some(result) = handles.tasks.join_next().await {
        match result {
            Ok(_) => tracing::warn!("Gateway component completed unexpectedly"),
            Err(e) => tracing::error!("Gateway component failed: {}", e),
        }

        if handles.tasks.is_empty() {
            tracing::error!("All gateway components stopped");
            break;
        }
    }
}

/// Main gateway execution function
///
/// This function implements the simplified gateway architecture where:
/// - Clients are fully self-managing (handle all transport concerns internally)
/// - No custom connection monitoring or health checking
/// - No complex timeout handling or retry logic
/// - Focus on component wiring and coordination only
///
/// The function is kept under 100 lines as required by the specification.
async fn run_gateway() -> Result<(), Box<dyn Error>> {
    let listener_ref = "example_listener".to_string();
    let server_uri = Uri::from_static("ws://localhost:9000");

    // Validate basic parameters
    validate_startup_parameters(&listener_ref, &server_uri)?;

    // Create self-managing clients with robust defaults
    // These clients handle all transport concerns internally
    let client = ConfigurationClient::connect(listener_ref.clone(), server_uri.clone()).await?;
    let events =
        ConfigurationEventsClient::connect(listener_ref.clone(), server_uri.clone()).await?;

    tracing::info!("🚀 Gateway clients initialized successfully");

    // Create and wire components
    let (components, events) = create_gateway_components(client, events).await?;

    // Start all components
    let handles = start_gateway_components(components, events).await?;

    tracing::info!("🚀 Gateway startup complete - all components running");

    // Wait for any component to stop (which shouldn't happen)
    wait_for_completion(handles).await;

    Ok(())
}
