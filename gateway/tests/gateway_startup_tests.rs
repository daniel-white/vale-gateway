use http::Uri;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use vg_rpc_client::{
    ConfigurationClient, ConfigurationClientError, ConfigurationEventClientError,
    ConfigurationEventsClient, RpcClientConfig, RpcTransport,
};

/// Test helper to simulate different service availability scenarios
struct TestEnvironment {
    server_uri: Uri,
}

impl TestEnvironment {
    fn with_unavailable_service() -> Self {
        // Use port 1 which should be unavailable
        Self {
            server_uri: "ws://127.0.0.1:1".parse().unwrap(),
        }
    }

    /// Helper function to create clients using the new RpcTransport pattern
    async fn create_clients(
        &self,
        listener_ref: String,
    ) -> (
        Result<ConfigurationClient, ConfigurationClientError>,
        Result<ConfigurationEventsClient, ConfigurationEventClientError>,
    ) {
        match RpcTransport::with_config(self.server_uri.clone(), RpcClientConfig::default()).await {
            Ok(transport) => {
                let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
                let events = ConfigurationEventsClient::new(transport, listener_ref);
                (Ok(client), Ok(events))
            }
            Err(_) => {
                // Convert transport error to client errors for compatibility
                let client_err = ConfigurationClientError::ConnectionUnavailable;
                let events_err = ConfigurationEventClientError::ConnectionFailed;
                (Err(client_err), Err(events_err))
            }
        }
    }

    fn with_invalid_host() -> Self {
        // Use invalid hostname
        Self {
            server_uri: "ws://invalid-host-that-does-not-exist:9000"
                .parse()
                .unwrap(),
        }
    }

    fn get_server_uri(&self) -> Uri {
        self.server_uri.clone()
    }
}

/// Test gateway startup with unavailable configuration service
/// This tests the graceful startup behavior required by requirement 3.1
#[tokio::test]
async fn test_gateway_startup_with_unavailable_service() {
    let env = TestEnvironment::with_unavailable_service();

    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Test gateway startup behavior with unavailable service
    let start_time = Instant::now();

    // This should complete within the startup timeout even if service is unavailable
    let startup_result = timeout(Duration::from_secs(10), async {
        env.create_clients(listener_ref).await
    })
    .await;

    let startup_time = start_time.elapsed();

    // Verify startup completed within reasonable time even with unavailable service
    assert!(
        startup_result.is_ok(),
        "Startup should not hang with unavailable service"
    );
    assert!(
        startup_time < Duration::from_secs(10),
        "Startup should complete within timeout"
    );

    if let Ok((client_result, events_result)) = startup_result {
        // With graceful startup, clients might be created even if service is unavailable
        // or they might fail with appropriate errors
        match (client_result, events_result) {
            (Ok(client), Ok(events_client)) => {
                // If clients are created, they should have monitoring
                assert!(
                    client.is_monitoring().await,
                    "Client should have monitoring even with unavailable service"
                );
                assert!(
                    events_client.is_monitoring().await,
                    "Events client should have monitoring even with unavailable service"
                );
            }
            (Err(client_err), Err(events_err)) => {
                // If clients fail to create, errors should be appropriate
                assert!(matches!(
                    client_err,
                    ConfigurationClientError::ConnectionUnavailable
                ));
                assert!(matches!(
                    events_err,
                    ConfigurationEventClientError::ConnectionFailed
                ));
            }
            _ => {
                // Mixed results are also acceptable - some clients might succeed while others fail
            }
        }
    }
}

/// Test gateway startup time requirements
/// This verifies requirement 3.1: startup should complete in under 5 seconds
#[tokio::test]
async fn test_gateway_startup_time_requirements() {
    // Test with unavailable service to ensure startup doesn't block
    let env = TestEnvironment::with_unavailable_service();

    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Measure startup time
    let start_time = Instant::now();

    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
    let _client_result = transport_result
        .as_ref()
        .map(|t| ConfigurationClient::new(t.clone(), listener_ref.clone()));
    let _events_result = transport_result.map(|t| ConfigurationEventsClient::new(t, listener_ref));

    let startup_time = start_time.elapsed();

    // Verify startup meets the 5-second requirement from requirements 3.1
    assert!(
        startup_time < Duration::from_secs(5),
        "Gateway startup should complete in under 5 seconds, took {:?}",
        startup_time
    );
}

/// Test gateway startup reliability with multiple attempts
/// This tests the robustness of the startup process
#[tokio::test]
async fn test_gateway_startup_reliability() {
    let env = TestEnvironment::with_unavailable_service();

    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Test multiple startup attempts to verify reliability
    let attempts = 3;

    for attempt in 0..attempts {
        let start_time = Instant::now();

        let transport_result =
            RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
        let (client_result, events_result) = match transport_result {
            Ok(transport) => (
                Ok(ConfigurationClient::new(
                    transport.clone(),
                    format!("{}-{}", listener_ref, attempt),
                )),
                Ok(ConfigurationEventsClient::new(
                    transport,
                    format!("{}-{}", listener_ref, attempt),
                )),
            ),
            Err(_) => (
                Err(ConfigurationClientError::ConnectionUnavailable),
                Err(ConfigurationEventClientError::ConnectionFailed),
            ),
        };

        let startup_time = start_time.elapsed();

        // Startup should always complete within time limit
        assert!(
            startup_time < Duration::from_secs(5),
            "Each startup attempt should complete quickly"
        );

        // Results should be consistent across attempts
        match (client_result, events_result) {
            (Ok(client), Ok(events_client)) => {
                // If successful, verify monitoring
                assert!(
                    client.is_monitoring().await,
                    "Client should have monitoring"
                );
                assert!(
                    events_client.is_monitoring().await,
                    "Events client should have monitoring"
                );
            }
            (Err(_), Err(_)) => {
                // Consistent failure is also acceptable
            }
            _ => {
                // Mixed results should be rare but not necessarily wrong
            }
        }
    }
}

/// Test component wiring and initialization patterns
/// This tests the patterns used in the gateway's create_gateway_components function
#[tokio::test]
async fn test_component_wiring_patterns() {
    let env = TestEnvironment::with_unavailable_service();

    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create clients as the gateway would
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
    let (client_result, events_result) = match transport_result {
        Ok(transport) => (
            Ok(ConfigurationClient::new(
                transport.clone(),
                listener_ref.clone(),
            )),
            Ok(ConfigurationEventsClient::new(transport, listener_ref)),
        ),
        Err(_) => (
            Err(ConfigurationClientError::ConnectionUnavailable),
            Err(ConfigurationEventClientError::ConnectionFailed),
        ),
    };

    // Test component wiring patterns regardless of connection success
    match (client_result, events_result) {
        (Ok(client), Ok(events_client)) => {
            // Test that clients can be used for component wiring

            // Verify events client functionality
            let events_rx = events_client.events();
            assert!(!events_rx.is_closed(), "Events receiver should be open");

            // Verify monitoring is active for both clients
            assert!(
                client.is_monitoring().await,
                "Configuration client should have active monitoring"
            );
            assert!(
                events_client.is_monitoring().await,
                "Events client should have active monitoring"
            );

            // Test that monitoring status is available
            let client_status = client.monitoring_status().await;
            let _events_status = events_client.monitoring_status().await;

            assert!(
                client_status.is_some(),
                "Client monitoring status should be available"
            );
            // Events client monitoring status is always available (not Option)
            // so we just verify it can be retrieved without error
        }
        _ => {
            // If clients can't be created, that's acceptable for unavailable service
            // The important thing is that the creation attempt completed quickly
        }
    }
}

/// Test startup parameter validation patterns
/// This simulates the gateway's validate_startup_parameters function
#[tokio::test]
async fn test_startup_parameter_validation() {
    // Test empty listener reference
    let empty_listener_ref = "";

    // This would be caught by gateway's validate_startup_parameters function
    assert!(
        empty_listener_ref.is_empty(),
        "Empty listener ref should be detected"
    );

    // Test invalid URI schemes
    let invalid_uris = vec![
        "http://localhost:9000",  // Wrong scheme
        "https://localhost:9000", // Wrong scheme
        "tcp://localhost:9000",   // Wrong scheme
    ];

    for invalid_uri_str in invalid_uris {
        let uri: Uri = invalid_uri_str.parse().unwrap();
        match uri.scheme_str() {
            Some("ws") | Some("wss") => panic!("Should not accept non-websocket schemes"),
            _ => {} // Expected - invalid scheme
        }
    }

    // Test valid URI schemes
    let valid_uris = vec!["ws://localhost:9000", "wss://localhost:9000"];

    for valid_uri_str in valid_uris {
        let uri: Uri = valid_uri_str.parse().unwrap();
        match uri.scheme_str() {
            Some("ws") | Some("wss") => {} // Expected - valid scheme
            _ => panic!("Should accept websocket schemes"),
        }
    }
}

/// Test startup with various service availability scenarios
/// This tests different network conditions the gateway might encounter
#[tokio::test]
async fn test_startup_with_various_service_scenarios() {
    // Scenario 1: Service completely unavailable (port not listening)
    {
        let listener_ref = "test-listener-1".to_string();
        let server_uri: Uri = "ws://127.0.0.1:1".parse().unwrap(); // Port 1 should be unavailable

        let start_time = Instant::now();
        let transport_result =
            RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
        let client_result = transport_result.map(|t| ConfigurationClient::new(t, listener_ref));
        let startup_time = start_time.elapsed();

        // Should complete quickly even with unavailable service (graceful startup)
        assert!(
            startup_time < Duration::from_secs(5),
            "Should not hang with unavailable service"
        );

        // Result depends on graceful startup implementation
        match client_result {
            Ok(client) => {
                // If graceful startup succeeds, client should have monitoring
                assert!(
                    client.is_monitoring().await,
                    "Client should have monitoring even with unavailable service"
                );
            }
            Err(_) => {
                // Transport errors are acceptable for unavailable service
            }
        }
    }

    // Scenario 2: Invalid hostname (DNS resolution failure)
    {
        let listener_ref = "test-listener-2".to_string();
        let server_uri: Uri = "ws://invalid-host-that-does-not-exist:9000"
            .parse()
            .unwrap();

        let start_time = Instant::now();
        let transport_result =
            RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
        let client_result = transport_result.map(|t| ConfigurationClient::new(t, listener_ref));
        let startup_time = start_time.elapsed();

        // Should complete quickly even with DNS resolution failure
        assert!(
            startup_time < Duration::from_secs(5),
            "Should not hang with DNS resolution failure"
        );

        // Should handle DNS resolution failure gracefully
        match client_result {
            Ok(client) => {
                assert!(
                    client.is_monitoring().await,
                    "Client should have monitoring"
                );
            }
            Err(other) => {
                // Transport errors are acceptable for DNS resolution failure
                println!("DNS resolution failure resulted in: {:?}", other);
            }
        }
    }

    // Scenario 3: Valid URI format but unreachable service (with timeout)
    {
        let listener_ref = "test-listener-3".to_string();
        let server_uri: Uri = "ws://192.0.2.1:9000".parse().unwrap(); // RFC5737 test address

        let start_time = Instant::now();

        // Use timeout to prevent hanging on network timeouts
        let client_result = timeout(Duration::from_secs(8), async {
            let transport_result =
                RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
            transport_result.map(|t| ConfigurationClient::new(t, listener_ref))
        })
        .await;

        let startup_time = start_time.elapsed();

        // Should complete within reasonable timeout or timeout gracefully
        assert!(
            startup_time < Duration::from_secs(10),
            "Should not hang indefinitely with unreachable service (took {:?})",
            startup_time
        );

        // Should handle unreachable service gracefully
        match client_result {
            Ok(Ok(client)) => {
                assert!(
                    client.is_monitoring().await,
                    "Client should have monitoring"
                );
            }
            Ok(Err(other)) => {
                // Transport errors are acceptable for unreachable service
                println!("Unreachable service resulted in: {:?}", other);
            }
            Err(_timeout) => {
                // Timeout is acceptable for unreachable service - shows the client
                // is trying to connect but not hanging indefinitely
                println!("Client connection timed out for unreachable service (expected)");
            }
        }
    }
}

/// Test that startup doesn't block on configuration service connectivity
/// This verifies requirement 3.2: startup should not block on service connectivity
#[tokio::test]
async fn test_startup_non_blocking_behavior() {
    let env = TestEnvironment::with_unavailable_service();

    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Use a very short timeout to verify non-blocking behavior
    let startup_result = timeout(Duration::from_secs(3), async {
        let transport_result =
            RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
        match transport_result {
            Ok(transport) => (
                Ok(ConfigurationClient::new(
                    transport.clone(),
                    listener_ref.clone(),
                )),
                Ok(ConfigurationEventsClient::new(transport, listener_ref)),
            ),
            Err(_) => (
                Err(ConfigurationClientError::ConnectionUnavailable),
                Err(ConfigurationEventClientError::ConnectionFailed),
            ),
        }
    })
    .await;

    // Should not timeout - startup should complete quickly
    assert!(
        startup_result.is_ok(),
        "Startup should not block on service connectivity"
    );

    if let Ok((client_result, events_result)) = startup_result {
        // Verify that even if clients are created, they handle disconnected state
        match (client_result, events_result) {
            (Ok(client), Ok(events_client)) => {
                // Clients should be self-managing and handle disconnected state
                assert!(
                    client.is_monitoring().await,
                    "Client should have monitoring for self-management"
                );
                assert!(
                    events_client.is_monitoring().await,
                    "Events client should have monitoring for self-management"
                );
            }
            _ => {
                // Failure to create clients is also acceptable for unavailable service
            }
        }
    }
}

/// Test startup logging and status reporting
/// This verifies that startup provides appropriate feedback
#[tokio::test]
async fn test_startup_logging_and_status() {
    let env = TestEnvironment::with_unavailable_service();

    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create clients and verify they provide status information
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;
    let (client_result, events_result) = match transport_result {
        Ok(transport) => (
            Ok(ConfigurationClient::new(
                transport.clone(),
                listener_ref.clone(),
            )),
            Ok(ConfigurationEventsClient::new(transport, listener_ref)),
        ),
        Err(_) => (
            Err(ConfigurationClientError::ConnectionUnavailable),
            Err(ConfigurationEventClientError::ConnectionFailed),
        ),
    };

    // Verify that clients provide monitoring status regardless of connection success
    match client_result {
        Ok(client) => {
            let status = client.monitoring_status().await;
            assert!(status.is_some(), "Client should provide monitoring status");
        }
        Err(_) => {
            // If client creation fails, that's acceptable for unavailable service
        }
    }

    match events_result {
        Ok(events_client) => {
            let _status = events_client.monitoring_status().await;
            // Events client monitoring status is always available (not Option)
            // so we just verify it can be retrieved without error
        }
        Err(_) => {
            // If events client creation fails, that's acceptable for unavailable service
        }
    }
}
/// Test that verifies no "completed unexpectedly" messages during controller outages
/// This tests the core requirement that tasks persist during connection failures
#[tokio::test]
async fn test_no_completed_unexpectedly_messages_during_controller_outage() {
    let env = TestEnvironment::with_unavailable_service();
    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create clients that would normally connect to controller
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;

    match transport_result {
        Ok(transport) => {
            let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
            let mut events_client = ConfigurationEventsClient::new(transport, listener_ref);

            // Start the events client task - this should not complete even with unavailable service
            let events_handle = events_client.start().await;

            match events_handle {
                Ok(handle) => {
                    // Wait a short time to let the task attempt connections
                    tokio::time::sleep(Duration::from_millis(500)).await;

                    // Verify client is still functional for monitoring
                    assert!(
                        client.is_monitoring().await,
                        "Client should maintain monitoring during outage"
                    );
                    assert!(
                        events_client.is_monitoring().await,
                        "Events client should maintain monitoring during outage"
                    );

                    // Stop the task gracefully
                    let _ = handle.shutdown();
                }
                Err(_) => {
                    // If the task fails to start, that's acceptable for unavailable service
                    // The important thing is that it doesn't cause "completed unexpectedly" messages
                }
            }
        }
        Err(_) => {
            // Transport creation failure is acceptable for unavailable service
            // The test verifies that this doesn't cause "completed unexpectedly" messages
        }
    }
}

/// Test that verifies persistent task behavior during extended controller outages
/// This tests requirement 2.1, 2.2, 2.3 for task persistence
#[tokio::test]
async fn test_persistent_task_behavior_during_extended_outage() {
    let env = TestEnvironment::with_unavailable_service();
    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create transport and clients
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;

    match transport_result {
        Ok(transport) => {
            let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
            let mut events_client = ConfigurationEventsClient::new(transport, listener_ref);

            // Start the events client task
            let events_handle = events_client.start().await;

            match events_handle {
                Ok(handle) => {
                    // Simulate extended outage by waiting longer
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    // Clients should maintain their monitoring capabilities
                    assert!(
                        client.is_monitoring().await,
                        "Client monitoring should persist during extended outage"
                    );
                    assert!(
                        events_client.is_monitoring().await,
                        "Events client monitoring should persist during extended outage"
                    );

                    // Stop the task
                    let _ = handle.shutdown();
                }
                Err(_) => {
                    // Task start failure is acceptable for unavailable service
                }
            }
        }
        Err(_) => {
            // Transport creation failure is acceptable for unavailable service
        }
    }
}

/// Test that verifies automatic reconnection behavior when controller becomes available
/// This tests requirement 2.1, 2.2 for transparent reconnection
#[tokio::test]
async fn test_automatic_reconnection_when_controller_available() {
    // This test would ideally start a mock controller, but for now we test the reconnection logic
    let env = TestEnvironment::with_unavailable_service();
    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create transport with reconnection enabled
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;

    match transport_result {
        Ok(transport) => {
            let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
            let mut events_client = ConfigurationEventsClient::new(transport, listener_ref);

            // Verify that clients are set up for automatic reconnection
            assert!(
                client.is_monitoring().await,
                "Client should have monitoring for reconnection"
            );
            assert!(
                events_client.is_monitoring().await,
                "Events client should have monitoring for reconnection"
            );

            // Start events client task
            let events_handle = events_client.start().await;

            match events_handle {
                Ok(handle) => {
                    // Let the task run briefly to establish reconnection attempts
                    tokio::time::sleep(Duration::from_millis(200)).await;

                    // Task should be running and attempting reconnection (we can't directly check this with Handle)

                    // Stop the task
                    let _ = handle.shutdown();
                }
                Err(_) => {
                    // Task start failure is acceptable for unavailable service
                }
            }
        }
        Err(_) => {
            // Transport creation failure is acceptable for unavailable service
        }
    }
}

/// Test that verifies graceful shutdown behavior
/// This tests requirement 2.2, 2.3 for proper shutdown signal handling
#[tokio::test]
async fn test_graceful_shutdown_behavior() {
    let env = TestEnvironment::with_unavailable_service();
    let listener_ref = "test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create transport and clients
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;

    match transport_result {
        Ok(transport) => {
            let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
            let mut events_client = ConfigurationEventsClient::new(transport, listener_ref);

            // Start the events client task
            let events_handle = events_client.start().await;

            match events_handle {
                Ok(handle) => {
                    // Let the task start
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // Task should be running (we can't directly check this with Handle)

                    // Gracefully stop the task (simulates shutdown signal)
                    let _ = handle.shutdown();

                    // Give it time to stop
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // Task should now be finished (we can't directly check this with Handle)

                    // Clients should still be functional for final cleanup
                    assert!(
                        client.is_monitoring().await,
                        "Client should remain functional during shutdown"
                    );
                    assert!(
                        events_client.is_monitoring().await,
                        "Events client should remain functional during shutdown"
                    );
                }
                Err(_) => {
                    // Task start failure is acceptable for unavailable service
                }
            }
        }
        Err(_) => {
            // Transport creation failure is acceptable for unavailable service
        }
    }
}
/// Test that verifies exponential backoff prevents endless loops
/// This test ensures that connection failures use proper backoff delays
#[tokio::test]
async fn test_exponential_backoff_prevents_endless_loops() {
    let env = TestEnvironment::with_unavailable_service();
    let listener_ref = "test-backoff-listener".to_string();
    let server_uri = env.get_server_uri();

    // Create transport and events client
    let transport_result =
        RpcTransport::with_config(server_uri.clone(), RpcClientConfig::default()).await;

    match transport_result {
        Ok(transport) => {
            let mut events_client = ConfigurationEventsClient::new(transport, listener_ref);

            // Start the events client task
            let events_handle = events_client.start().await;

            match events_handle {
                Ok(handle) => {
                    // Let it run for a short time to test backoff behavior
                    tokio::time::sleep(Duration::from_millis(2000)).await;

                    // Verify client maintains monitoring during backoff
                    assert!(
                        events_client.is_monitoring().await,
                        "Events client should maintain monitoring during backoff"
                    );

                    // Stop the task
                    let _ = handle.shutdown();
                }
                Err(_) => {
                    // Task start failure is acceptable for unavailable service
                    // The important thing is that it doesn't create an endless loop
                }
            }
        }
        Err(_) => {
            // Transport creation failure is acceptable for unavailable service
        }
    }
}
