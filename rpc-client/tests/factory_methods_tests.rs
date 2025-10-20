mod mock_server;

use mock_server::{MockConfigurationServer, ServerBehavior};
use std::time::Duration;
use vg_config::http::listener::{Listener, ListenerRef};
use vg_rpc_client::{
    ConfigurationClient, ConfigurationClientError, ConfigurationEventClientError,
    ConfigurationEventsClient, ErrorClassification,
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

/// Test ConfigurationClient::connect() factory method with normal server
#[tokio::test]
async fn test_configuration_client_connect_success() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Test connect() factory method
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect(listener_ref.clone(), uri)
        .await
        .expect("Failed to create client with connect()");

    // Verify client works
    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    // Verify monitoring is integrated
    assert!(
        client.is_monitoring().await,
        "Internal monitoring should be active"
    );

    // Verify monitoring status is available
    let status = client.monitoring_status().await;
    assert!(status.is_some(), "Monitoring status should be available");

    server.stop().await.expect("Failed to stop server");
}

/// Test ConfigurationClient::connect() with graceful startup when server is unavailable
#[tokio::test]
async fn test_configuration_client_connect_graceful_startup() {
    // Use an invalid URI to simulate unavailable service
    let listener_ref = ListenerRef::from("test-listener".to_string());
    let uri: http::Uri = "ws://127.0.0.1:1".parse().unwrap(); // Port 1 should be unavailable

    // connect() should handle unavailable service gracefully
    let result = ConfigurationClient::connect(listener_ref, uri).await;

    // In graceful startup mode, this might succeed with a client that handles disconnected state
    // or fail with a clear error message
    match result {
        Ok(client) => {
            // If successful, verify monitoring is still integrated
            assert!(
                client.is_monitoring().await,
                "Monitoring should be active even with graceful startup"
            );
        }
        Err(ConfigurationClientError::ConnectionUnavailable) => {
            // This is acceptable for graceful startup - the error should be clear
        }
        Err(other) => {
            panic!("Unexpected error type for graceful startup: {:?}", other);
        }
    }
}

/// Test ConfigurationClient::connect() error handling and fallback behavior
#[tokio::test]
async fn test_configuration_client_connect_error_handling() {
    let mut server =
        MockConfigurationServer::with_behavior(ServerBehavior::Failing { error_rate: 1.0 });
    let addr = server.start().await.expect("Failed to start server");

    let listener_ref = ListenerRef::from("test-listener".to_string());
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // connect() should handle server errors gracefully
    let result = ConfigurationClient::connect(listener_ref, uri).await;

    // Should either succeed with robust error handling or fail gracefully
    match result {
        Ok(client) => {
            // If successful, verify robust configuration is applied
            assert!(client.is_monitoring().await, "Monitoring should be active");

            // The client should handle errors internally
            let listener_result = client.listener().await;
            // With robust configuration, this might succeed due to retries or fail with handled error
            match listener_result {
                Ok(_) => {} // Success due to robust retry logic
                Err(e) => {
                    // Error should be properly classified
                    assert!(
                        e.is_retryable() || e.is_temporary(),
                        "Error should be retryable or temporary: {:?}",
                        e
                    );
                }
            }
        }
        Err(ConfigurationClientError::ConnectionUnavailable) => {
            // Acceptable for graceful startup with failing server
        }
        Err(other) => {
            panic!("Unexpected error for robust client: {:?}", other);
        }
    }

    server.stop().await.expect("Failed to stop server");
}

/// Test ConfigurationEventsClient::connect() factory method with normal server
#[tokio::test]
async fn test_configuration_events_client_connect_success() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Test connect() factory method for events client
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let events_client = ConfigurationEventsClient::connect(listener_ref.clone(), uri)
        .await
        .expect("Failed to create events client with connect()");

    // Verify monitoring is integrated for events client
    assert!(
        events_client.is_monitoring().await,
        "Internal monitoring should be active for events client"
    );

    // Verify monitoring status is available
    let status = events_client.monitoring_status().await;
    assert!(
        status.is_some(),
        "Monitoring status should be available for events client"
    );

    // Verify events receiver can be created
    let _events_rx = events_client.events();

    server.stop().await.expect("Failed to stop server");
}

/// Test ConfigurationEventsClient::connect() with graceful startup
#[tokio::test]
async fn test_configuration_events_client_connect_graceful_startup() {
    // Use an invalid URI to simulate unavailable service
    let listener_ref = ListenerRef::from("test-listener".to_string());
    let uri: http::Uri = "ws://127.0.0.1:1".parse().unwrap(); // Port 1 should be unavailable

    // connect() should handle unavailable service gracefully for events client
    let result = ConfigurationEventsClient::connect(listener_ref, uri).await;

    match result {
        Ok(client) => {
            // If successful, verify monitoring is integrated
            assert!(
                client.is_monitoring().await,
                "Monitoring should be active for events client"
            );
        }
        Err(ConfigurationEventClientError::ConnectionFailed) => {
            // This is acceptable for graceful startup
        }
        Err(other) => {
            panic!(
                "Unexpected error type for events client graceful startup: {:?}",
                other
            );
        }
    }
}

/// Test proper configuration setup in factory methods
#[tokio::test]
async fn test_factory_methods_configuration_setup() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Test ConfigurationClient configuration
    let client = ConfigurationClient::connect(listener_ref.clone(), uri.clone())
        .await
        .expect("Failed to create client");

    // Verify robust configuration is applied
    assert!(client.is_monitoring().await, "Monitoring should be enabled");

    let _status = client
        .monitoring_status()
        .await
        .expect("Should have monitoring status");
    // The status should indicate active monitoring
    // Note: Specific status fields depend on MonitoringStatus implementation

    // Test ConfigurationEventsClient configuration
    let events_client = ConfigurationEventsClient::connect(listener_ref.clone(), uri)
        .await
        .expect("Failed to create events client");

    // Verify events client has optimized configuration
    assert!(
        events_client.is_monitoring().await,
        "Events client monitoring should be enabled"
    );

    let _events_status = events_client
        .monitoring_status()
        .await
        .expect("Should have events monitoring status");
    // Events client should have monitoring optimized for event streams

    server.stop().await.expect("Failed to stop server");
}

/// Test monitoring integration in factory methods
#[tokio::test]
async fn test_factory_methods_monitoring_integration() {
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

    // Verify monitoring is active for both clients
    assert!(
        client.is_monitoring().await,
        "Client monitoring should be active"
    );
    assert!(
        events_client.is_monitoring().await,
        "Events client monitoring should be active"
    );

    // Test monitoring can be stopped
    client
        .stop_monitoring()
        .await
        .expect("Should be able to stop client monitoring");
    events_client
        .stop_monitoring()
        .await
        .expect("Should be able to stop events client monitoring");

    // Verify monitoring is stopped
    assert!(
        !client.is_monitoring().await,
        "Client monitoring should be stopped"
    );
    assert!(
        !events_client.is_monitoring().await,
        "Events client monitoring should be stopped"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test factory methods with slow server response
#[tokio::test]
async fn test_factory_methods_with_slow_server() {
    let mut server = MockConfigurationServer::with_behavior(ServerBehavior::Slow {
        delay: Duration::from_millis(100),
    });
    let addr = server.start().await.expect("Failed to start server");

    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Factory methods should handle slow servers gracefully
    let start_time = std::time::Instant::now();

    let client = ConfigurationClient::connect(listener_ref.clone(), uri.clone())
        .await
        .expect("Failed to create client with slow server");

    let events_client = ConfigurationEventsClient::connect(listener_ref, uri)
        .await
        .expect("Failed to create events client with slow server");

    let elapsed = start_time.elapsed();

    // Should complete within reasonable time (robust configuration should handle delays)
    assert!(
        elapsed < Duration::from_secs(10),
        "Factory methods should complete within reasonable time"
    );

    // Verify clients work despite slow server
    assert!(
        client.is_monitoring().await,
        "Client should have monitoring despite slow server"
    );
    assert!(
        events_client.is_monitoring().await,
        "Events client should have monitoring despite slow server"
    );

    server.stop().await.expect("Failed to stop server");
}

/// Test error handling and fallback behavior in factory methods
#[tokio::test]
async fn test_factory_methods_error_handling_and_fallback() {
    // Test with completely unavailable server
    let listener_ref = ListenerRef::from("test-listener".to_string());
    let uri: http::Uri = "ws://127.0.0.1:1".parse().unwrap(); // Port 1 should be unavailable

    // Both factory methods should handle unavailable servers gracefully
    let client_result = ConfigurationClient::connect(listener_ref.clone(), uri.clone()).await;
    let events_result = ConfigurationEventsClient::connect(listener_ref, uri).await;

    // Results should be consistent with graceful startup behavior
    match client_result {
        Ok(client) => {
            // If successful, should have monitoring
            assert!(
                client.is_monitoring().await,
                "Client should have monitoring even with unavailable server"
            );
        }
        Err(ConfigurationClientError::ConnectionUnavailable) => {
            // Acceptable error for unavailable server
        }
        Err(other) => {
            panic!("Unexpected client error: {:?}", other);
        }
    }

    match events_result {
        Ok(events_client) => {
            // If successful, should have monitoring
            assert!(
                events_client.is_monitoring().await,
                "Events client should have monitoring even with unavailable server"
            );
        }
        Err(ConfigurationEventClientError::ConnectionFailed) => {
            // Acceptable error for unavailable server
        }
        Err(other) => {
            panic!("Unexpected events client error: {:?}", other);
        }
    }
}
