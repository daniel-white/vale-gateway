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
use vg_rpc_client::{ConfigurationClient, ConfigurationEventsClient, RpcTransport};

#[cfg(not(unix))]
use tokio::signal;

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

/// Determine if an error is related to graceful shutdown
///
/// This function analyzes error messages to distinguish between expected shutdown
/// scenarios and unexpected failures, enabling appropriate logging levels.
fn is_shutdown_related_error(error_msg: &str) -> bool {
    let shutdown_indicators = [
        "cancelled",
        "shutdown",
        "stopped",
        "terminated",
        "closed",
        "aborted",
        "interrupted",
    ];

    let error_lower = error_msg.to_lowercase();
    shutdown_indicators
        .iter()
        .any(|indicator| error_lower.contains(indicator))
}

/// Setup shutdown signal handling for graceful termination
///
/// This function sets up handlers for SIGTERM and SIGINT signals to enable
/// graceful shutdown of gateway components when requested.
#[allow(dead_code)]
async fn setup_shutdown_signal() -> Result<(), Box<dyn Error + Send + Sync>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!(
                    target: "vg_gateway::shutdown",
                    signal = "SIGTERM",
                    "Received SIGTERM signal, initiating graceful shutdown"
                );
            }
            _ = sigint.recv() => {
                tracing::info!(
                    target: "vg_gateway::shutdown",
                    signal = "SIGINT",
                    "Received SIGINT signal, initiating graceful shutdown"
                );
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, only handle Ctrl+C
        signal::ctrl_c().await?;
        tracing::info!(
            target: "vg_gateway::shutdown",
            signal = "CTRL_C",
            "Received Ctrl+C signal, initiating graceful shutdown"
        );
    }

    Ok(())
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
    mut events: ConfigurationEventsClient,
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

async fn run_gateway() -> Result<(), Box<dyn Error>> {
    let listener_ref = "example_listener".to_string();
    let server_uri = Uri::from_static("ws://localhost:9000");

    // Validate basic parameters
    validate_startup_parameters(&listener_ref, &server_uri)?;

    // Create shared RPC transport with simple configuration
    // This single transport is shared between both configuration and events clients
    let transport = RpcTransport::new(server_uri.clone())
        .await
        .expect("Failed to create RPC transport");

    // Create clients using the shared transport
    let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
    let events = ConfigurationEventsClient::new(transport, listener_ref.clone());

    tracing::info!("🚀 Gateway clients initialized successfully using shared RPC transport");

    // Create and wire components
    let (components, events) = create_gateway_components(client, events).await?;

    // Start all components
    let mut handles = start_gateway_components(components, events).await?;

    tracing::info!("🚀 Gateway startup complete - all components running");

    // Setup shutdown signal handling
    let mut shutdown_signal = tokio::spawn(setup_shutdown_signal());

    // Wait for either component completion or shutdown signal
    // The robust transport handles all reconnection internally
    loop {
        tokio::select! {
            // Handle component completion/failure
            result = handles.tasks.join_next() => {
                match result {
                    Some(Ok(_)) => {
                        // Component completed normally - this should only happen during graceful shutdown
                        tracing::info!(
                            target: "vg_gateway::lifecycle",
                            event = "component_completed",
                            reason = "normal_completion",
                            "Gateway component completed normally during shutdown"
                        );
                    }
                    Some(Err(e)) => {
                        // Component failed with an error - determine if this is expected or unexpected
                        let error_msg = e.to_string();

                        // Check if this is a shutdown-related error (expected)
                        if is_shutdown_related_error(&error_msg) {
                            tracing::info!(
                                target: "vg_gateway::lifecycle",
                                event = "component_completed",
                                reason = "graceful_shutdown",
                                error = %e,
                                "Gateway component stopped during graceful shutdown"
                            );
                        } else {
                            // This is an unexpected failure
                            tracing::error!(
                                target: "vg_gateway::lifecycle",
                                event = "component_failed",
                                reason = "unexpected_error",
                                error = %e,
                                "Gateway component failed unexpectedly"
                            );
                            return Err(e.into());
                        }
                    }
                    None => {
                        // All tasks have completed
                        tracing::info!(
                            target: "vg_gateway::lifecycle",
                            event = "all_components_completed",
                            "All gateway components have completed"
                        );
                        break;
                    }
                }
            }
            // Handle shutdown signal
            shutdown_result = &mut shutdown_signal => {
                match shutdown_result {
                    Ok(Ok(())) => {
                        tracing::info!(
                            target: "vg_gateway::lifecycle",
                            event = "shutdown_signal_received",
                            "Shutdown signal received, stopping all components"
                        );

                        // Abort all tasks for graceful shutdown
                        handles.tasks.abort_all();

                        // Wait a brief moment for tasks to clean up
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                        tracing::info!(
                            target: "vg_gateway::lifecycle",
                            event = "graceful_shutdown_initiated",
                            "All components signaled to stop"
                        );
                        break;
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            target: "vg_gateway::lifecycle",
                            event = "shutdown_signal_error",
                            error = %e,
                            "Error setting up shutdown signal handler, continuing without signal handling"
                        );
                        // Continue without signal handling
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "vg_gateway::lifecycle",
                            event = "shutdown_task_error",
                            error = %e,
                            "Shutdown signal task failed, continuing without signal handling"
                        );
                        // Continue without signal handling
                    }
                }
            }
        }
    }

    tracing::info!(
        target: "vg_gateway::lifecycle",
        event = "gateway_shutdown",
        "All gateway components have stopped - shutdown complete"
    );
    Ok(())
}
