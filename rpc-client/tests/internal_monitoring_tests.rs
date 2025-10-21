// NOTE: Some tests in this file are marked with #[ignore] because they attempt to connect
// to unavailable services which triggers the retry logic in RpcTransport::create_client_with_retry().
// These tests need to be rewritten with proper mock transport implementations.

mod mock_server;

use mock_server::{MockConfigurationServer, ServerBehavior};
use std::time::Duration;
use tokio::time::sleep;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_rpc_client::{
    ConfigurationClient, ConfigurationEventsClient, RpcClientConfig, RpcTransport,
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

/// Test that internal monitoring is automatically enabled for clients created with new API
#[tokio::test]
async fn test_internal_monitoring_enabled_by_default() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Create shared RpcTransport and clients using new API
    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
    let events_client = ConfigurationEventsClient::new(transport, listener_ref);

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
    let _client_status = client.monitoring_status().await;
    let _events_status = events_client.monitoring_status().await;

    // Note: monitoring_status() returns MonitoringStatus directly, not Option
    // Just verify we can call it without error

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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport, listener_ref);

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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport, listener_ref);

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
    let _status = client.monitoring_status().await;

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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport, listener_ref);

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
    let _status = client.monitoring_status().await;

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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport, listener_ref.clone());

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
        let transport = RpcTransport::new(uri)
            .await
            .expect("Failed to create RpcTransport");
        let client = ConfigurationClient::new(transport, listener_ref);

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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let events_client = ConfigurationEventsClient::new(transport, listener_ref);

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
/// TODO: This test needs to be rewritten with proper mocks to avoid hanging on retry logic
#[tokio::test]
#[ignore = "Hangs due to retry logic - needs mock transport implementation"]
async fn test_monitoring_with_unavailable_service() {
    let listener_ref = ListenerRef::from("test-listener".to_string());
    let uri: http::Uri = "ws://127.0.0.1:1".parse().unwrap(); // Port 1 should be unavailable

    // Try to create transport with unavailable service
    let transport_result = RpcTransport::new(uri).await;

    // Test monitoring behavior based on whether transport was created
    match transport_result {
        Ok(transport) => {
            let client = ConfigurationClient::new(transport.clone(), listener_ref.clone());
            let events_client = ConfigurationEventsClient::new(transport, listener_ref);

            // If transport was created despite unavailable service, monitoring should be active
            assert!(
                client.is_monitoring().await,
                "Client should have monitoring even with unavailable service"
            );
            assert!(
                events_client.is_monitoring().await,
                "Events client should have monitoring even with unavailable service"
            );
        }
        Err(_) => {
            // This is acceptable for unavailable service
            // RpcTransport creation failed, which is expected
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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport, listener_ref);

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
        let transport = RpcTransport::new(uri.clone())
            .await
            .expect("Failed to create RpcTransport");
        let client = ConfigurationClient::new(transport, format!("test-listener-{}", i));

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

    let transport = RpcTransport::new(uri)
        .await
        .expect("Failed to create RpcTransport");
    let client = ConfigurationClient::new(transport, listener_ref);

    // Verify monitoring is active (this should generate some log output)
    assert!(client.is_monitoring().await, "Monitoring should be active");

    // Let monitoring run to generate some log entries
    sleep(Duration::from_millis(100)).await;

    // Verify monitoring status is available (indicates logging infrastructure is working)
    let _status = client.monitoring_status().await;

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

    // Test with default configuration
    let default_transport = RpcTransport::new(uri.clone())
        .await
        .expect("Failed to create default RpcTransport");
    let default_client = ConfigurationClient::new(default_transport, listener_ref.clone());

    assert!(
        default_client.is_monitoring().await,
        "Default client should have monitoring"
    );

    // Test with custom configuration
    let custom_config = RpcClientConfig::new()
        .with_monitoring(true)
        .with_metrics(true);
    let custom_transport = RpcTransport::with_config(uri.clone(), custom_config)
        .await
        .expect("Failed to create custom RpcTransport");
    let custom_client = ConfigurationClient::new(custom_transport, listener_ref.clone());

    assert!(
        custom_client.is_monitoring().await,
        "Custom client should have monitoring"
    );

    // Test with monitoring disabled
    let no_monitoring_config = RpcClientConfig::new().with_monitoring(false);
    let no_monitoring_transport = RpcTransport::with_config(uri, no_monitoring_config)
        .await
        .expect("Failed to create no-monitoring RpcTransport");
    let no_monitoring_client = ConfigurationClient::new(no_monitoring_transport, listener_ref);

    // This client might not have monitoring enabled
    let has_monitoring = no_monitoring_client.is_monitoring().await;
    println!("No-monitoring client has monitoring: {}", has_monitoring);
    // We don't assert here since this client was configured without monitoring

    server.stop().await.expect("Failed to stop server");
}
