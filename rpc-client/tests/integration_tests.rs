mod mock_server;

use mock_server::{MockConfigurationServer, ServerBehavior};
use std::time::Duration;

use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};
use vg_rpc_client::{
    CircuitBreakerConfig, ConfigurationClient, ConfigurationClientError, RetryPolicy,
    RobustClientConfig, TimeoutConfig,
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

fn create_test_route() -> (RouteRef, Route) {
    let route_ref = RouteRef::from("test-route".to_string());
    let route = Route::builder()
        .ref_(route_ref.clone())
        .host_matchers(Vec::new())
        .rules(Vec::new())
        .build();
    (route_ref, route)
}

fn create_test_backend() -> (BackendRef, Backend) {
    let backend_ref = BackendRef::from("test-backend".to_string());
    let backend = Backend::builder()
        .ref_(backend_ref.clone())
        .endpoints(Vec::new())
        .build();
    (backend_ref, backend)
}

fn create_test_shared_filter() -> (SharedFilterRef, SharedFilter) {
    use vg_config::http::filter::access_control::{
        AccessControlFilter, AccessControlFilterRef, AccessControlSharedFilter,
    };

    let filter_ref =
        SharedFilterRef::AccessControl(AccessControlFilterRef::from("test-filter".to_string()));
    let access_control_filter = AccessControlSharedFilter::builder()
        .ref_(AccessControlFilterRef::from("test-filter".to_string()))
        .filter(
            AccessControlFilter::builder()
                .effect(vg_config::http::filter::access_control::AccessControlEffect::Allow)
                .clients(Vec::new())
                .build(),
        )
        .build();
    let filter = SharedFilter::AccessControl(access_control_filter);
    (filter_ref, filter)
}

/// Test basic client functionality with normal server behavior
#[tokio::test]
async fn test_basic_client_functionality() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data to server
    let (listener_ref, listener) = create_test_listener();
    let (route_ref, route) = create_test_route();
    let (backend_ref, backend) = create_test_backend();
    let (filter_ref, filter) = create_test_shared_filter();

    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;
    server.add_route(route_ref.clone(), route.clone()).await;
    server
        .add_backend(backend_ref.clone(), backend.clone())
        .await;
    server
        .add_shared_filter(filter_ref.clone(), filter.clone())
        .await;

    // Create client
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect(listener_ref.clone(), uri)
        .await
        .expect("Failed to create client");

    // Test all API methods
    let result_listener = client.listener().await.expect("Failed to get listener");
    assert_eq!(result_listener.ref_(), &listener_ref);

    let result_route = client.route(&route_ref).await.expect("Failed to get route");
    assert_eq!(result_route.ref_(), route_ref);

    let result_backend = client
        .backend(&backend_ref)
        .await
        .expect("Failed to get backend");
    assert_eq!(result_backend.ref_(), backend_ref);

    let result_filter = client
        .shared_filter(&filter_ref)
        .await
        .expect("Failed to get shared filter");
    assert_eq!(result_filter.ref_(), filter_ref);

    // Verify request counts
    assert_eq!(server.get_request_count("listener").await, 1);
    assert_eq!(server.get_request_count("route").await, 1);
    assert_eq!(server.get_request_count("backend").await, 1);
    assert_eq!(server.get_request_count("shared_filter").await, 1);

    server.stop().await.expect("Failed to stop server");
}

/// Test client with production robustness configuration
#[tokio::test]
async fn test_production_client_configuration() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Create client with production configuration
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_production(listener_ref.clone(), uri)
        .await
        .expect("Failed to create production client");

    // Test that client works with production configuration
    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    server.stop().await.expect("Failed to stop server");
}

/// Test client with development robustness configuration
#[tokio::test]
async fn test_development_client_configuration() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Create client with development configuration
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_development(listener_ref.clone(), uri)
        .await
        .expect("Failed to create development client");

    // Test that client works with development configuration
    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    server.stop().await.expect("Failed to stop server");
}

/// Test timeout middleware functionality
#[tokio::test]
async fn test_timeout_middleware() {
    let mut server = MockConfigurationServer::with_behavior(ServerBehavior::Slow {
        delay: Duration::from_secs(2),
    });
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Create client with short timeout
    let timeout_config = TimeoutConfig::builder()
        .default_timeout(Duration::from_millis(500))
        .build();

    let robust_config = RobustClientConfig::builder()
        .timeout(Some(timeout_config))
        .build();

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_with_config(listener_ref.clone(), uri, robust_config)
        .await
        .expect("Failed to create client");

    // Test that request times out
    let result = client.listener().await;
    match result {
        Err(ConfigurationClientError::RequestTimeout(_)) => {
            // Expected timeout error
        }
        other => panic!("Expected timeout error, got: {:?}", other),
    }

    server.stop().await.expect("Failed to stop server");
}

/// Test retry middleware functionality
#[tokio::test]
async fn test_retry_middleware() {
    let mut server = MockConfigurationServer::with_behavior(ServerBehavior::Failing {
        error_rate: 0.8, // 80% failure rate
    });
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Create client with retry configuration
    let retry_policy = RetryPolicy::builder()
        .max_attempts(5)
        .base_delay(Duration::from_millis(10))
        .max_delay(Duration::from_millis(100))
        .build();

    let robust_config = RobustClientConfig::builder()
        .retry(Some(retry_policy))
        .build();

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_with_config(listener_ref.clone(), uri, robust_config)
        .await
        .expect("Failed to create client");

    // Make multiple requests - some should eventually succeed due to retries
    let mut success_count = 0;
    let mut retry_exhausted_count = 0;

    for _ in 0..10 {
        match client.listener().await {
            Ok(_) => success_count += 1,
            Err(ConfigurationClientError::MaxRetriesExceeded(_)) => retry_exhausted_count += 1,
            Err(other) => panic!("Unexpected error: {:?}", other),
        }
    }

    // With 80% failure rate and 5 retries, we should see some successes
    // and some retry exhaustions
    assert!(success_count > 0 || retry_exhausted_count > 0);

    // Verify that retries were attempted
    let total_requests = server.get_request_count("listener").await;
    assert!(total_requests > 10); // Should be more than 10 due to retries

    server.stop().await.expect("Failed to stop server");
}

/// Test circuit breaker middleware functionality
#[tokio::test]
async fn test_circuit_breaker_middleware() {
    let mut server = MockConfigurationServer::with_behavior(ServerBehavior::Failing {
        error_rate: 1.0, // 100% failure rate
    });
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Create client with circuit breaker configuration
    let circuit_breaker_config = CircuitBreakerConfig::builder()
        .failure_threshold(3)
        .success_threshold(2)
        .timeout(Duration::from_millis(100))
        .minimum_throughput(2)
        .build();

    let robust_config = RobustClientConfig::builder()
        .circuit_breaker(Some(circuit_breaker_config))
        .build();

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_with_config(listener_ref.clone(), uri, robust_config)
        .await
        .expect("Failed to create client");

    // Make requests to trigger circuit breaker
    let mut circuit_breaker_errors = 0;
    let mut _other_errors = 0;

    for _ in 0..10 {
        match client.listener().await {
            Err(ConfigurationClientError::CircuitBreakerOpen) => circuit_breaker_errors += 1,
            Err(_) => _other_errors += 1,
            Ok(_) => panic!("Unexpected success with 100% failure rate"),
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // After enough failures, circuit breaker should open
    assert!(circuit_breaker_errors > 0);

    server.stop().await.expect("Failed to stop server");
}

/// Test middleware composition and interaction
#[tokio::test]
async fn test_middleware_composition() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    let (route_ref, route) = create_test_route();
    let (backend_ref, backend) = create_test_backend();
    let (filter_ref, filter) = create_test_shared_filter();

    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;
    server.add_route(route_ref.clone(), route.clone()).await;
    server
        .add_backend(backend_ref.clone(), backend.clone())
        .await;
    server
        .add_shared_filter(filter_ref.clone(), filter.clone())
        .await;

    // Create client with multiple middleware layers
    let timeout_config = TimeoutConfig::builder()
        .default_timeout(Duration::from_secs(1))
        .build();

    let retry_policy = RetryPolicy::builder()
        .max_attempts(3)
        .base_delay(Duration::from_millis(10))
        .max_delay(Duration::from_millis(100))
        .build();

    let circuit_breaker_config = CircuitBreakerConfig::builder()
        .failure_threshold(5)
        .success_threshold(2)
        .timeout(Duration::from_millis(500))
        .minimum_throughput(2)
        .build();

    let robust_config = RobustClientConfig::builder()
        .timeout(Some(timeout_config))
        .retry(Some(retry_policy))
        .circuit_breaker(Some(circuit_breaker_config))
        .build();

    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_with_config(listener_ref.clone(), uri, robust_config)
        .await
        .expect("Failed to create client");

    // Test all methods work with middleware stack
    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    let result = client.route(&route_ref).await.expect("Failed to get route");
    assert_eq!(result.ref_(), route_ref);

    let result = client
        .backend(&backend_ref)
        .await
        .expect("Failed to get backend");
    assert_eq!(result.ref_(), backend_ref);

    let result = client
        .shared_filter(&filter_ref)
        .await
        .expect("Failed to get shared filter");
    assert_eq!(result.ref_(), filter_ref);

    server.stop().await.expect("Failed to stop server");
}

/// Test error handling and propagation through middleware stack
#[tokio::test]
async fn test_error_handling_propagation() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let listener_ref = ListenerRef::from("nonexistent-listener".to_string());

    // Create client with all middleware enabled
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::connect_production(listener_ref.clone(), uri)
        .await
        .expect("Failed to create client");

    // Test NotFound error propagation
    let result = client.listener().await;
    match result {
        Err(ConfigurationClientError::NotFound) => {
            // Expected NotFound error
        }
        other => panic!("Expected NotFound error, got: {:?}", other),
    }

    server.stop().await.expect("Failed to stop server");
}

/// Test client builder pattern with different configurations
#[tokio::test]
async fn test_client_builder_configurations() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    // Add test data
    let (listener_ref, listener) = create_test_listener();
    server
        .add_listener(listener_ref.clone(), listener.clone())
        .await;

    // Test builder with timeout
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();
    let client = ConfigurationClient::builder()
        .connect_with_timeout(listener_ref.clone(), uri.clone(), Duration::from_secs(5))
        .await
        .expect("Failed to create client with timeout");

    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    // Test builder with circuit breaker
    let client = ConfigurationClient::builder()
        .connect_with_circuit_breaker(
            listener_ref.clone(),
            uri.clone(),
            5,                          // failure_threshold
            2,                          // success_threshold
            Duration::from_millis(500), // timeout
        )
        .await
        .expect("Failed to create client with circuit breaker");

    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    // Test builder with retry
    let client = ConfigurationClient::builder()
        .connect_with_retry(
            listener_ref.clone(),
            uri,
            3,                          // max_attempts
            Duration::from_millis(10),  // base_delay
            Duration::from_millis(100), // max_delay
        )
        .await
        .expect("Failed to create client with retry");

    let result = client.listener().await.expect("Failed to get listener");
    assert_eq!(result.ref_(), &listener_ref);

    server.stop().await.expect("Failed to stop server");
}

/// Test configuration validation
#[tokio::test]
async fn test_configuration_validation() {
    let mut server = MockConfigurationServer::new();
    let addr = server.start().await.expect("Failed to start server");

    let listener_ref = ListenerRef::from("test-listener".to_string());
    let uri: http::Uri = format!("ws://127.0.0.1:{}", addr.port()).parse().unwrap();

    // Test invalid timeout configuration (zero timeout)
    let result = ConfigurationClient::builder()
        .connect_with_timeout(
            listener_ref.clone(),
            uri.clone(),
            Duration::from_secs(0), // Invalid: zero timeout
        )
        .await;

    assert!(result.is_err());

    // Test invalid circuit breaker configuration (zero thresholds)
    let result = ConfigurationClient::builder()
        .connect_with_circuit_breaker(
            listener_ref.clone(),
            uri.clone(),
            0, // Invalid: zero failure threshold
            0, // Invalid: zero success threshold
            Duration::from_millis(500),
        )
        .await;

    assert!(result.is_err());

    // Test invalid retry configuration (zero attempts)
    let result = ConfigurationClient::builder()
        .connect_with_retry(
            listener_ref.clone(),
            uri,
            0, // Invalid: zero max attempts
            Duration::from_millis(10),
            Duration::from_millis(100),
        )
        .await;

    assert!(result.is_err());

    server.stop().await.expect("Failed to stop server");
}
