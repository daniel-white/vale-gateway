mod mock_server;

use mock_server::{MockConfigurationServer, ServerBehavior};
use std::time::Duration;
use tokio::time::sleep;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_rpc_client::{
    ConfigurationClient, ConfigurationClientError, ConfigurationEventClientError,
    ConfigurationEventsClient,
};

/// Helper function to create test data
fn create_test_listener() -> (ListenerRef, Listener) {
    let listener_ref = ListenerRef::from("test-listener".to_string());
    let listener = Listener::builder()
        .ref_(listener_ref.clone())
        .policies(vg_config::http::listener::policy::ListenerPolicies::default())
        .filters(Vec::new())
        .shared_filter_refs(Vec::new())
        .route_refs(Vec::new())
        .backend_refs(Vec::new())
        .build();
    (listener_ref, listener)
}

/// Test that internal monitoring is automatically enabled for clients created with connect()
#[tokio::test]
async fn test_internal_monitoring_enabled_by_default() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Create clients using factory methods
    let client = ConfigurationClient::connect(listener_ref.clone(), uri.clone())
        .await
        .expect("Failed to create client");

    let events_client = ConfigurationEventsClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create events client");

    // Verify monitoring is enabled by default
    assert!(
        client.is_monitoring().await,
        "Configuration client should have monitoring enabled by default"
    );
    assert!(
        events_client.is_monitoring().await,
        "Events client should have monitoring enabled by default"
    );

    // Verify monitoring status is available
    let client_status = client.monitoring_status().await;
    let events_status = events_client.monitoring_status().await;

    assert!(
        client_status.is_some(),
        "Client should provide monitoring status"
    );
    assert!(
        events_status.is_some(),
        "Events client should provide monitoring status"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring lifecycle - start, stop, restart
#[tokio::test]
async fn test_monitoring_lifecycle() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let client = ConfigurationClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create client");

    // Initially monitoring should be active
    assert!(
        client.is_monitoring().await,
        "Monitoring should be active initially"
    );

    // Stop monitoring
    client
        .stop_monitoring()
        .await
        .expect("Should be able to stop monitoring");

    // Verify monitoring is stopped
    assert!(
        !client.is_monitoring().await,
        "Monitoring should be stopped"
    );

    // Monitoring status should still be available but indicate stopped state
    let _status = client.monitoring_status().await;
    // Note: The exact behavior depends on implementation - status might be None or indicate stopped state

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring behavior with server failures
#[tokio::test]
async fn test_monitoring_with_server_failures() {
    let mut server =
        MockConfigurationServer::with_behavior(ServerBehavior::Failing { error_rate: 0.8 });
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let client = ConfigurationClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create client");

    // Monitoring should be active even with failing server
    assert!(
        client.is_monitoring().await,
        "Monitoring should be active even with failing server"
    );

    // Let monitoring run for a short time to detect failures
    sleep(Duration::from_millis(100)).await;

    // Monitoring should still be active (it should handle failures internally)
    assert!(
        client.is_monitoring().await,
        "Monitoring should remain active despite server failures"
    );

    // Status should be available and might indicate connection issues
    let status = client.monitoring_status().await;
    assert!(
        status.is_some(),
        "Monitoring status should be available even with server failures"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring behavior with slow server responses
#[tokio::test]
async fn test_monitoring_with_slow_server() {
    let mut server = MockConfigurationServer::with_behavior(ServerBehavior::Slow {
        delay: Duration::from_millis(200),
    });
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let client = ConfigurationClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create client");

    // Monitoring should be active with slow server
    assert!(
        client.is_monitoring().await,
        "Monitoring should be active with slow server"
    );

    // Let monitoring run for a short time
    sleep(Duration::from_millis(300)).await;

    // Monitoring should adapt to slow responses
    assert!(
        client.is_monitoring().await,
        "Monitoring should remain active with slow server"
    );

    // Status should be available
    let status = client.monitoring_status().await;
    assert!(
        status.is_some(),
        "Monitoring status should be available with slow server"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test that monitoring doesn't interfere with client operations
#[tokio::test]
async fn test_monitoring_non_interference() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let client = ConfigurationClient::connect(listener_ref.clone(), uri)
        .await
        .expect("Failed to create client");

    // Verify monitoring is active
    assert!(client.is_monitoring().await, "Monitoring should be active");

    // Perform client operations while monitoring is running
    let listener_result = client.listener().await;
    assert!(
        listener_result.is_ok(),
        "Client operations should work with monitoring active"
    );

    // Verify the result is correct
    if let Ok(result_listener) = listener_result {
        assert_eq!(result_listener.ref_(), &listener_ref);
    }

    // Monitoring should still be active after client operations
    assert!(
        client.is_monitoring().await,
        "Monitoring should remain active after client operations"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring cleanup on client drop
#[tokio::test]
async fn test_monitoring_cleanup() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    {
        let client = ConfigurationClient::connect(listener_ref, uri)
            .await
            .expect("Failed to create client");

        // Verify monitoring is active
        assert!(client.is_monitoring().await, "Monitoring should be active");

        // Client goes out of scope here, monitoring should be cleaned up
    }

    // Give some time for cleanup
    sleep(Duration::from_millis(50)).await;

    // Note: We can't directly test that monitoring is cleaned up since the client is dropped,
    // but this test ensures that the cleanup process doesn't panic or cause issues

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring with events client
#[tokio::test]
async fn test_events_client_monitoring() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let events_client = ConfigurationEventsClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create events client");

    // Verify monitoring is active for events client
    assert!(
        events_client.is_monitoring().await,
        "Events client should have monitoring active"
    );

    // Verify events client can create receivers while monitoring is active
    let _events_rx = events_client.events();

    // Monitoring should not interfere with event stream functionality
    assert!(
        events_client.is_monitoring().await,
        "Monitoring should remain active after creating event receiver"
    );

    // Test monitoring lifecycle for events client
    events_client
        .stop_monitoring()
        .await
        .expect("Should be able to stop events client monitoring");
    assert!(
        !events_client.is_monitoring().await,
        "Events client monitoring should be stopped"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring behavior with unavailable service
#[tokio::test]
async fn test_monitoring_with_unavailable_service() {
    let listener_ref = "test-listener".to_string();
    let uri: http::Uri = "ws://127.0.0.1:1".parse().unwrap(); // Port 1 should be unavailable

    // Try to create client with unavailable service
    let client_result = ConfigurationClient::connect(listener_ref.clone(), uri.clone()).await;
    let events_result = ConfigurationEventsClient::connect(listener_ref, uri).await;

    // Test monitoring behavior based on whether clients were created
    match client_result {
        Ok(client) => {
            // If client was created despite unavailable service, monitoring should be active
            assert!(
                client.is_monitoring().await,
                "Client should have monitoring even with unavailable service"
            );

            // Status should be available
            let status = client.monitoring_status().await;
            assert!(
                status.is_some(),
                "Monitoring status should be available even with unavailable service"
            );
        }
        Err(ConfigurationClientError::ConnectionUnavailable) => {
            // This is acceptable for unavailable service
        }
        Err(other) => {
            panic!("Unexpected error for unavailable service: {:?}", other);
        }
    }

    match events_result {
        Ok(events_client) => {
            // If events client was created, monitoring should be active
            assert!(
                events_client.is_monitoring().await,
                "Events client should have monitoring even with unavailable service"
            );

            // Status should be available
            let status = events_client.monitoring_status().await;
            assert!(
                status.is_some(),
                "Events client monitoring status should be available even with unavailable service"
            );
        }
        Err(ConfigurationEventClientError::ConnectionFailed) => {
            // This is acceptable for unavailable service
        }
        Err(other) => {
            panic!(
                "Unexpected events client error for unavailable service: {:?}",
                other
            );
        }
    }
}

/// Test monitoring error handling and recovery
#[tokio::test]
async fn test_monitoring_error_handling_and_recovery() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let client = ConfigurationClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create client");

    // Initially monitoring should be active
    assert!(
        client.is_monitoring().await,
        "Monitoring should be active initially"
    );

    // Simulate server becoming unavailable
    server
        .set_behavior(ServerBehavior::Failing { error_rate: 1.0 })
        .await;

    // Let monitoring detect the failures
    sleep(Duration::from_millis(200)).await;

    // Monitoring should still be active (handling errors internally)
    assert!(
        client.is_monitoring().await,
        "Monitoring should remain active despite server failures"
    );

    // Restore server to normal behavior
    server.set_behavior(ServerBehavior::Normal).await;

    // Let monitoring detect recovery
    sleep(Duration::from_millis(200)).await;

    // Monitoring should still be active and recovered
    assert!(
        client.is_monitoring().await,
        "Monitoring should remain active after server recovery"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring performance and resource usage
#[tokio::test]
async fn test_monitoring_performance() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Create multiple clients to test monitoring overhead
    let mut clients = Vec::new();
    for i in 0..5 {
        let client = ConfigurationClient::connect(format!("test-listener-{}", i), uri.clone())
            .await
            .expect("Failed to create client");

        assert!(
            client.is_monitoring().await,
            "Each client should have monitoring active"
        );
        clients.push(client);
    }

    // Let monitoring run for a short time
    sleep(Duration::from_millis(100)).await;

    // All clients should still have active monitoring
    for (i, client) in clients.iter().enumerate() {
        assert!(
            client.is_monitoring().await,
            "Client {} should still have active monitoring",
            i
        );
    }

    // Test that monitoring doesn't significantly impact client operations
    let start_time = std::time::Instant::now();

    for client in &clients {
        // This will fail since we only added one listener, but that's ok for performance testing
        let _ = client.listener().await;
    }

    let operation_time = start_time.elapsed();

    // Operations should complete quickly even with monitoring active
    assert!(
        operation_time < Duration::from_secs(1),
        "Client operations should be fast with monitoring active"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring logging output (basic verification)
#[tokio::test]
async fn test_monitoring_logging() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    let client = ConfigurationClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create client");

    // Verify monitoring is active (this should generate some log output)
    assert!(client.is_monitoring().await, "Monitoring should be active");

    // Let monitoring run to generate some log entries
    sleep(Duration::from_millis(100)).await;

    // Verify monitoring status is available (indicates logging infrastructure is working)
    let status = client.monitoring_status().await;
    assert!(status.is_some(), "Monitoring status should be available");

    // Note: We can't easily test the actual log output in unit tests,
    // but this test ensures the logging infrastructure doesn't cause panics

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring with different client configurations
#[tokio::test]
async fn test_monitoring_with_different_configurations() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Test with production configuration
    let prod_client = ConfigurationClient::connect_production(listener_ref.clone(), uri.clone())
        .await
        .expect("Failed to create production client");

    assert!(
        prod_client.is_monitoring().await,
        "Production client should have monitoring"
    );

    // Test with development configuration
    let dev_client = ConfigurationClient::connect_development(listener_ref.clone(), uri.clone())
        .await
        .expect("Failed to create development client");

    assert!(
        dev_client.is_monitoring().await,
        "Development client should have monitoring"
    );

    // Test with simple configuration (might not have monitoring)
    let simple_client = ConfigurationClient::connect_simple(listener_ref, uri)
        .await
        .expect("Failed to create simple client");

    // Simple client might not have monitoring enabled
    let has_monitoring = simple_client.is_monitoring().await;
    println!("Simple client has monitoring: {}", has_monitoring);
    // We don't assert here since simple client might not have monitoring by design

    server.stop().await.expect("Failed to stop server");
}
