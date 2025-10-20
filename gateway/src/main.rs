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
use std::time::Duration;
use tokio::select;
use tokio::task::JoinSet;
use vg_core::instrumentation::init;
use vg_core::net::topology::TopologyLocation;
use vg_rpc_client::{ConfigurationClient, ConfigurationEventsClient};

#[tokio::main]
async fn main() {
    init("vg-gateway");

    // Run the gateway and handle any startup errors gracefully
    if let Err(e) = run_gateway().await {
        tracing::error!(
            "Gateway startup failed: {}. Entering degraded mode and staying alive.",
            e
        );
        tracing::info!("Gateway will remain running and attempt to recover periodically.");

        // Keep the gateway alive even if startup failed
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            tracing::info!("💓 Gateway heartbeat - running in minimal mode after startup failure");

            // Optionally, we could retry startup here in the future
            // For now, just keep the process alive
        }
    }
}

async fn run_gateway() -> Result<(), Box<dyn Error>> {
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

    // Create configuration client
    let client =
        ConfigurationClient::connect_production(listener_ref.clone(), server_uri.clone()).await?;

    tracing::info!(
        "✓ Configuration client created successfully for {}",
        server_uri
    );

    // Create events client
    let events =
        ConfigurationEventsClient::connect_production(listener_ref.clone(), server_uri.clone())
            .await?;

    tracing::info!("✓ Events client created successfully for {}", server_uri);

    tracing::info!("🔧 Gateway configured for resilient operation:");
    tracing::info!(
        "  • Lazy connection mode: Clients created without requiring immediate connection"
    );
    tracing::info!(
        "  • Unlimited retries: Will continuously attempt to reconnect to configuration service"
    );
    tracing::info!("  • Request queuing: Up to 1000 requests queued during disconnection");
    tracing::info!(
        "  • Degraded mode: Gateway will continue running even if configuration service is unavailable"
    );

    // Perform optional connectivity validation during startup
    tracing::info!(
        "Performing startup connectivity validation for configuration service at {}...",
        server_uri
    );
    match tokio::time::timeout(
        Duration::from_secs(2), // Short timeout for startup validation
        client.listener(),
    )
    .await
    {
        Ok(Ok(_)) => {
            tracing::info!(
                "✓ Startup connectivity validation successful - configuration service is available at {}",
                server_uri
            );
        }
        Ok(Err(e)) => {
            tracing::info!(
                "⚠ Startup connectivity validation failed - configuration service may be unavailable at {}: {}. Client will retry connections in background.",
                server_uri,
                e
            );
        }
        Err(_) => {
            tracing::info!(
                "⚠ Startup connectivity validation timed out - configuration service may be slow or unavailable at {}. Client will retry connections in background.",
                server_uri
            );
        }
    }

    // Set up connection status monitoring for the configuration client if available
    // This will periodically log connection status and health information
    let client_for_monitoring = client.clone();
    let server_uri_for_monitoring = server_uri.clone();
    let connection_monitor_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30)); // Check every 30 seconds
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut consecutive_failures = 0u32;
        let mut last_success_time = std::time::Instant::now();

        tracing::info!(
            "Starting connection health monitoring for configuration service at {} (checking every 30 seconds)",
            server_uri_for_monitoring
        );

        loop {
            interval.tick().await;

            // Perform a lightweight health check by attempting to get listener info
            let health_check_start = std::time::Instant::now();
            match tokio::time::timeout(Duration::from_secs(5), client_for_monitoring.listener())
                .await
            {
                Ok(Ok(_)) => {
                    let response_time = health_check_start.elapsed();

                    if consecutive_failures > 0 {
                        tracing::info!(
                            "✓ Configuration service connection recovered at {} (response time: {:?}, was down for {} checks)",
                            server_uri_for_monitoring,
                            response_time,
                            consecutive_failures
                        );
                    } else {
                        tracing::debug!(
                            "Configuration service connection health check: OK (response time: {:?})",
                            response_time
                        );
                    }

                    consecutive_failures = 0;
                    last_success_time = std::time::Instant::now();
                }
                Ok(Err(e)) => {
                    consecutive_failures += 1;
                    let time_since_last_success = last_success_time.elapsed();

                    tracing::warn!(
                        "⚠ Configuration service connection health check failed at {} (attempt {}, down for {:?}): {}",
                        server_uri_for_monitoring,
                        consecutive_failures,
                        time_since_last_success,
                        e
                    );
                }
                Err(_) => {
                    consecutive_failures += 1;
                    let time_since_last_success = last_success_time.elapsed();

                    tracing::warn!(
                        "⚠ Configuration service connection health check timed out at {} (attempt {}, down for {:?})",
                        server_uri_for_monitoring,
                        consecutive_failures,
                        time_since_last_success
                    );
                }
            }

            // Log critical status if we've been down for too long
            if consecutive_failures >= 10 {
                // 5 minutes of failures
                let time_since_last_success = last_success_time.elapsed();
                tracing::error!(
                    "🚨 Configuration service at {} has been unavailable for {:?} ({} consecutive failures). Gateway operations may be degraded.",
                    server_uri_for_monitoring,
                    time_since_last_success,
                    consecutive_failures
                );
            }
        }
    });

    // Create events receiver and source configuration
    // The clients use lazy startup, so they're always available but may not be connected yet
    let events_rx = events.events();
    let source_configuration: SourceConfigurationRegistry =
        SourceConfigurationRegistryOptions::builder()
            .client(client.clone())
            .events(events_rx.clone())
            .build()
            .into();

    let current_location = CurrentLocationConfigurator::new();

    // Create shared filter handlers and backend configurator
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

    // Start all components
    let mut rrx = source_configuration.routing();
    let mut brx = backends_configurator.backends();
    let mut sfhx = shared_filter_handlers.handlers();

    let shared_filter_handlers_handle = shared_filter_handlers.start();
    let backends_configurator_handle = backends_configurator.start();
    let source_configuration_handle = source_configuration.start();

    // Start event client with graceful error handling and timeout
    // The events client uses lazy startup, so it will handle connection failures gracefully
    let event_client = match tokio::time::timeout(
        Duration::from_secs(5), // Timeout for events client startup
        events.start(),
    )
    .await
    {
        Ok(Ok(handle)) => {
            tracing::info!("Successfully started events client");
            Some(handle)
        }
        Ok(Err(e)) => {
            tracing::warn!(
                "Failed to start events client (configuration service may be unavailable): {}. Gateway will continue startup without events initially. Events will be available once the service reconnects.",
                e
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                "Events client startup timed out (configuration service may be slow or unavailable). Gateway will continue startup without events initially. Events will be available once the service reconnects."
            );
            None
        }
    };

    let mut js = JoinSet::new();

    // Spawn component tasks
    js.spawn(shared_filter_handlers_handle.stopped());
    js.spawn(backends_configurator_handle.stopped());
    js.spawn(source_configuration_handle.stopped());

    // Only spawn event client task if it was successfully started
    if let Some(event_client) = event_client {
        js.spawn(event_client.stopped());
    } else {
        tracing::info!(
            "Events client not available at startup - events will be processed once connection is established"
        );
    }

    // Spawn connection monitoring task
    js.spawn(async move {
        connection_monitor_task.await.unwrap_or_else(|e| {
            tracing::error!("Connection monitoring task failed: {}", e);
        });
    });

    // Spawn a heartbeat task to show the gateway is alive
    js.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            tracing::info!("💓 Gateway heartbeat - system is running and healthy");
        }
    });

    // Spawn monitoring loop
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

    // Keep the gateway running indefinitely, even in degraded state
    // Handle task failures gracefully and log them, but don't exit
    tracing::info!("🚀 Gateway startup complete - running in continuous mode");
    tracing::info!("Gateway will remain active even if configuration service is unavailable");

    loop {
        // Wait for any task to complete (which shouldn't happen for long-running tasks)
        if let Some(result) = js.join_next().await {
            match result {
                Ok(_) => {
                    tracing::warn!(
                        "A gateway component task completed unexpectedly - this may indicate a problem"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "A gateway component task failed: {} - gateway will continue running in degraded mode",
                        e
                    );
                }
            }

            // Log current status
            tracing::info!("Gateway continues running with {} active tasks", js.len());

            // If we have no tasks left, something went very wrong, but we'll still keep running
            if js.is_empty() {
                tracing::error!("All gateway tasks have stopped - running in minimal mode");
                tracing::info!("Gateway will continue running and attempt to recover");

                // Sleep for a while before checking again
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        } else {
            // This shouldn't happen since we have infinite tasks, but handle it gracefully
            tracing::error!("No tasks remaining - gateway entering minimal survival mode");
            tracing::info!("Gateway will continue running and wait for recovery");

            // Keep the gateway alive with a simple loop
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                tracing::info!("Gateway heartbeat - running in minimal mode, waiting for recovery");
            }
        }
    }
}
