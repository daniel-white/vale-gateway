use jsonrpsee::core::client::Error as JsonRpcError;
use jsonrpsee::types::ErrorObjectOwned;
use std::error::Error;
use std::time::Duration;
use vg_rpc_client::{
    CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerLayer, ConfigValidationError,
    ConfigurationClientError, ConnectionError, ErrorClassification, ExponentialBackoffPolicy,
    ReconnectionConfig, ReconnectionLayer, RetryExhaustedError, RetryLayer, RetryPolicy,
    RobustClientConfig, SourceError, TimeoutConfig, TimeoutLayer, WsClientLayer,
};

/// Test comprehensive error handling validation
/// Verifies all error paths work correctly with middleware stack
/// Tests error propagation through all layers
#[cfg(test)]
mod error_handling_validation_tests {
    use super::*;

    #[test]
    fn test_configuration_client_error_classification() {
        // Test retryable errors
        assert!(ConfigurationClientError::RequestTimeout(Duration::from_secs(30)).is_retryable());
        assert!(ConfigurationClientError::ConnectionUnavailable.is_retryable());
        assert!(ConfigurationClientError::ServiceUnavailable.is_retryable());
        assert!(
            ConfigurationClientError::TransportError(SourceError::from(Box::new(
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test")
            )
                as Box<dyn std::error::Error + Send + Sync>))
            .is_retryable()
        );

        // Test non-retryable errors
        assert!(!ConfigurationClientError::NotFound.is_retryable());
        assert!(!ConfigurationClientError::CircuitBreakerOpen.is_retryable());
        assert!(!ConfigurationClientError::MaxRetriesExceeded(3).is_retryable());
        assert!(
            !ConfigurationClientError::ConfigurationError(ConfigValidationError::InvalidTimeout(
                "test".to_string()
            ))
            .is_retryable()
        );
        assert!(
            !ConfigurationClientError::Unknown(SourceError::from(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "test"
            ))
                as Box<dyn std::error::Error + Send + Sync>))
            .is_retryable()
        );
    }

    #[test]
    fn test_circuit_breaker_triggering_errors() {
        // Test errors that should trigger circuit breaker
        assert!(
            ConfigurationClientError::RequestTimeout(Duration::from_secs(30))
                .should_trip_circuit_breaker()
        );
        assert!(ConfigurationClientError::ConnectionUnavailable.should_trip_circuit_breaker());
        assert!(ConfigurationClientError::ServiceUnavailable.should_trip_circuit_breaker());
        assert!(
            ConfigurationClientError::TransportError(SourceError::from(Box::new(
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test")
            )
                as Box<dyn std::error::Error + Send + Sync>))
            .should_trip_circuit_breaker()
        );
        assert!(ConfigurationClientError::MaxRetriesExceeded(3).should_trip_circuit_breaker());
        assert!(
            ConfigurationClientError::Unknown(SourceError::from(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "test"
            ))
                as Box<dyn std::error::Error + Send + Sync>))
            .should_trip_circuit_breaker()
        );

        // Test errors that should not trigger circuit breaker
        assert!(!ConfigurationClientError::NotFound.should_trip_circuit_breaker());
        assert!(!ConfigurationClientError::CircuitBreakerOpen.should_trip_circuit_breaker());
        assert!(
            !ConfigurationClientError::ConfigurationError(ConfigValidationError::InvalidTimeout(
                "test".to_string()
            ))
            .should_trip_circuit_breaker()
        );
    }

    #[test]
    fn test_temporary_error_classification() {
        // Test temporary errors
        assert!(ConfigurationClientError::RequestTimeout(Duration::from_secs(30)).is_temporary());
        assert!(ConfigurationClientError::ConnectionUnavailable.is_temporary());
        assert!(ConfigurationClientError::ServiceUnavailable.is_temporary());
        assert!(
            ConfigurationClientError::TransportError(SourceError::from(Box::new(
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test")
            )
                as Box<dyn std::error::Error + Send + Sync>))
            .is_temporary()
        );
        assert!(ConfigurationClientError::CircuitBreakerOpen.is_temporary()); // Can recover

        // Test permanent errors
        assert!(!ConfigurationClientError::NotFound.is_temporary());
        assert!(!ConfigurationClientError::MaxRetriesExceeded(3).is_temporary());
        assert!(
            !ConfigurationClientError::ConfigurationError(ConfigValidationError::InvalidTimeout(
                "test".to_string()
            ))
            .is_temporary()
        );
        assert!(
            !ConfigurationClientError::Unknown(SourceError::from(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "test"
            ))
                as Box<dyn std::error::Error + Send + Sync>))
            .is_temporary()
        );
    }

    #[test]
    fn test_error_conversions_from_jsonrpc_errors() {
        // Test timeout error conversion
        let timeout_error = JsonRpcError::RequestTimeout;
        let client_error: ConfigurationClientError = timeout_error.into();
        match client_error {
            ConfigurationClientError::RequestTimeout(_) => {} // Expected
            _ => panic!("Expected RequestTimeout error"),
        }

        // Test transport error conversion
        let transport_error = JsonRpcError::Transport(Box::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        )));
        let client_error: ConfigurationClientError = transport_error.into();
        match client_error {
            ConfigurationClientError::TransportError(_) => {} // Expected
            _ => panic!("Expected TransportError"),
        }

        // Test call error conversion (not found)
        let not_found_error = ErrorObjectOwned::owned(-32601, "Method not found", None::<()>);
        let call_error = JsonRpcError::Call(not_found_error);
        let client_error: ConfigurationClientError = call_error.into();
        // This should be converted to Unknown since it's not specifically handled
        match client_error {
            ConfigurationClientError::Unknown(_) => {} // Expected
            _ => panic!("Expected Unknown error for unhandled call error"),
        }
    }

    #[test]
    fn test_tower_timeout_error_conversion() {
        let timeout_elapsed = tower::timeout::error::Elapsed::new();
        let client_error: ConfigurationClientError = timeout_elapsed.into();
        match client_error {
            ConfigurationClientError::RequestTimeout(_) => {} // Expected
            _ => panic!("Expected RequestTimeout error"),
        }
    }

    #[test]
    fn test_circuit_breaker_error_conversion() {
        let cb_error = CircuitBreakerError;
        let client_error: ConfigurationClientError = cb_error.into();
        match client_error {
            ConfigurationClientError::CircuitBreakerOpen => {} // Expected
            _ => panic!("Expected CircuitBreakerOpen error"),
        }
    }

    #[test]
    fn test_retry_exhausted_error_conversion() {
        let retry_error = RetryExhaustedError { attempts: 5 };
        let client_error: ConfigurationClientError = retry_error.into();
        match client_error {
            ConfigurationClientError::MaxRetriesExceeded(5) => {} // Expected
            _ => panic!("Expected MaxRetriesExceeded error with 5 attempts"),
        }
    }

    #[test]
    fn test_connection_error_conversion() {
        let conn_error = ConnectionError {
            message: "Connection failed".to_string(),
        };
        let client_error: ConfigurationClientError = conn_error.into();
        match client_error {
            ConfigurationClientError::ConnectionUnavailable => {} // Expected
            _ => panic!("Expected ConnectionUnavailable error"),
        }
    }

    #[test]
    fn test_configuration_validation_error_propagation() {
        // Test timeout validation error
        let timeout_config = TimeoutConfig::builder()
            .default_timeout(Duration::ZERO) // Invalid
            .build();

        let validation_result = timeout_config.validate();
        assert!(validation_result.is_err());

        let validation_error = validation_result.unwrap_err();
        let client_error: ConfigurationClientError = validation_error.into();
        match client_error {
            ConfigurationClientError::ConfigurationError(_) => {} // Expected
            _ => panic!("Expected ConfigurationError"),
        }

        // Test circuit breaker validation error
        let cb_config = CircuitBreakerConfig::builder()
            .failure_threshold(0) // Invalid
            .build();

        let validation_result = cb_config.validate();
        assert!(validation_result.is_err());

        // Test retry policy validation error
        let retry_config = RetryPolicy::builder()
            .max_attempts(0) // Invalid
            .build();

        let validation_result = retry_config.validate();
        assert!(validation_result.is_err());

        // Test reconnection config validation error
        let reconnect_config = ReconnectionConfig::builder()
            .reconnect_base_delay(Duration::ZERO) // Invalid
            .build();

        let validation_result = reconnect_config.validate();
        assert!(validation_result.is_err());
    }

    #[test]
    fn test_robust_client_config_validation_error_propagation() {
        // Test that invalid sub-configurations are caught by the main config validation
        let invalid_timeout_config = TimeoutConfig::builder()
            .default_timeout(Duration::ZERO) // Invalid
            .build();

        let robust_config = RobustClientConfig::builder()
            .timeout(Some(invalid_timeout_config))
            .build();

        let validation_result = robust_config.validate();
        assert!(validation_result.is_err());

        match validation_result.unwrap_err() {
            ConfigValidationError::InvalidTimeout(_) => {} // Expected
            _ => panic!("Expected InvalidTimeout validation error"),
        }
    }

    #[test]
    fn test_error_chain_preservation() {
        // Test that error chains are preserved through conversions
        let source_error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test");
        let boxed_error = Box::new(source_error) as Box<dyn std::error::Error + Send + Sync>;
        let source_error = SourceError::from(boxed_error);
        let client_error = ConfigurationClientError::TransportError(source_error);

        // Verify error message is preserved
        assert!(client_error.to_string().contains("Transport error"));

        // Verify source error is accessible
        assert!(client_error.source().is_some());
    }

    #[test]
    fn test_layer_error_handling_integration() {
        // Test timeout layer error handling
        let timeout_layer = TimeoutLayer::new(Duration::from_secs(30));
        assert!(timeout_layer.is_enabled());
        assert_eq!(timeout_layer.layer_name(), "timeout");

        // Test retry layer error handling
        let retry_policy = RetryPolicy::default();
        let retry_layer = RetryLayer::new(retry_policy);
        assert!(retry_layer.is_enabled());
        assert_eq!(retry_layer.layer_name(), "retry");

        // Test circuit breaker layer error handling
        let cb_config = CircuitBreakerConfig::default();
        let cb_layer = CircuitBreakerLayer::new(cb_config);
        assert!(cb_layer.is_enabled());
        assert_eq!(cb_layer.layer_name(), "circuit-breaker");

        // Test reconnection layer error handling
        let reconnect_config = ReconnectionConfig::default();
        let reconnect_layer = ReconnectionLayer::new(reconnect_config);
        assert!(reconnect_layer.is_enabled());
        assert_eq!(reconnect_layer.layer_name(), "reconnection");
    }

    #[test]
    fn test_exponential_backoff_policy_error_classification() {
        let policy = ExponentialBackoffPolicy::new(RetryPolicy::default());

        // Test retryable errors
        assert!(ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::RequestTimeout(Duration::from_secs(30))
        ));
        assert!(ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::ConnectionUnavailable
        ));
        assert!(ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::ServiceUnavailable
        ));

        // Test non-retryable errors
        assert!(!ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::NotFound
        ));
        assert!(!ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::CircuitBreakerOpen
        ));
        assert!(!ExponentialBackoffPolicy::is_retryable_error(
            &ConfigurationClientError::MaxRetriesExceeded(3)
        ));
    }

    #[test]
    fn test_circuit_breaker_error_classification() {
        // Test jsonrpsee error classification for circuit breaker

        // Transport errors should trigger circuit breaker
        assert!(CircuitBreakerLayer::classify_error(
            &JsonRpcError::Transport(Box::new(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "test"
            )))
        ));

        // Request timeout should trigger circuit breaker
        assert!(CircuitBreakerLayer::classify_error(
            &JsonRpcError::RequestTimeout
        ));

        // Service disconnect should trigger circuit breaker
        assert!(CircuitBreakerLayer::classify_error(
            &JsonRpcError::ServiceDisconnect
        ));

        // Server errors should trigger circuit breaker
        let server_error = ErrorObjectOwned::owned(-32000, "Internal error", None::<()>);
        assert!(CircuitBreakerLayer::classify_error(&JsonRpcError::Call(
            server_error
        )));

        // Client errors should not trigger circuit breaker
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::InvalidSubscriptionId
        ));
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::HttpNotImplemented
        ));

        // Parse errors should not trigger circuit breaker
        let parse_error = serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "parse error",
        ));
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::ParseError(parse_error)
        ));

        // Custom errors should trigger circuit breaker (conservative approach)
        assert!(CircuitBreakerLayer::classify_error(&JsonRpcError::Custom(
            "custom error".to_string()
        )));
    }

    #[test]
    fn test_error_display_formatting() {
        // Test that all error types have proper Display implementations
        let errors = vec![
            ConfigurationClientError::NotFound,
            ConfigurationClientError::RequestTimeout(Duration::from_secs(30)),
            ConfigurationClientError::CircuitBreakerOpen,
            ConfigurationClientError::ConnectionUnavailable,
            ConfigurationClientError::MaxRetriesExceeded(3),
            ConfigurationClientError::ServiceUnavailable,
            ConfigurationClientError::ConfigurationError(ConfigValidationError::InvalidTimeout(
                "test".to_string(),
            )),
            ConfigurationClientError::TransportError(SourceError::from(Box::new(
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test"),
            )
                as Box<dyn std::error::Error + Send + Sync>)),
            ConfigurationClientError::Unknown(SourceError::from(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "test",
            ))
                as Box<dyn std::error::Error + Send + Sync>)),
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(
                !error_string.is_empty(),
                "Error should have non-empty display string"
            );
            assert!(
                !error_string.contains("Error"),
                "Error display should not contain generic 'Error' text"
            );
        }
    }

    #[test]
    fn test_error_debug_formatting() {
        // Test that all error types have proper Debug implementations
        let errors = vec![
            ConfigurationClientError::NotFound,
            ConfigurationClientError::RequestTimeout(Duration::from_secs(30)),
            ConfigurationClientError::CircuitBreakerOpen,
            ConfigurationClientError::ConnectionUnavailable,
            ConfigurationClientError::MaxRetriesExceeded(3),
            ConfigurationClientError::ServiceUnavailable,
        ];

        for error in errors {
            let debug_string = format!("{:?}", error);
            assert!(
                !debug_string.is_empty(),
                "Error should have non-empty debug string"
            );
        }
    }

    #[test]
    fn test_source_error_wrapper() {
        // Test SourceError wrapper functionality
        let io_error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test");
        let boxed_error = Box::new(io_error) as Box<dyn std::error::Error + Send + Sync>;
        let source_error = SourceError::from(boxed_error);

        // Test Display
        let display_string = source_error.to_string();
        assert!(!display_string.is_empty());

        // Test Debug
        let debug_string = format!("{:?}", source_error);
        assert!(!debug_string.is_empty());

        // Test Error trait
        assert!(std::error::Error::source(&source_error).is_none()); // SourceError doesn't chain
    }

    #[test]
    fn test_configuration_validation_error_types() {
        // Test all configuration validation error types
        let validation_errors = vec![
            ConfigValidationError::InvalidTimeout("test timeout".to_string()),
            ConfigValidationError::InvalidCircuitBreaker("test circuit breaker".to_string()),
            ConfigValidationError::InvalidRetryPolicy("test retry policy".to_string()),
            ConfigValidationError::InvalidReconnection("test reconnection".to_string()),
        ];

        for error in validation_errors {
            let error_string = error.to_string();
            assert!(
                !error_string.is_empty(),
                "Validation error should have non-empty display string"
            );

            let debug_string = format!("{:?}", error);
            assert!(
                !debug_string.is_empty(),
                "Validation error should have non-empty debug string"
            );
        }
    }

    #[test]
    fn test_error_compatibility_with_std_error() {
        // Test that our errors work well with standard error handling
        fn test_function() -> Result<(), Box<dyn std::error::Error>> {
            let error = ConfigurationClientError::NotFound;
            Err(Box::new(error))
        }

        let result = test_function();
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Resource not found"));
    }
}

/// Integration tests for error propagation through middleware layers
#[cfg(test)]
mod error_propagation_integration_tests {
    use super::*;

    use vg_rpc_client::layers::LayeredClient;
    use vg_rpc_client::{NoOpLayer, WsClientLayer};

    // Mock layer for testing error propagation
    struct ErrorTestLayer {
        should_error: bool,
        error_type: ConfigurationClientError,
    }

    impl ErrorTestLayer {
        fn new(should_error: bool, error_type: ConfigurationClientError) -> Self {
            Self {
                should_error,
                error_type,
            }
        }
    }

    impl WsClientLayer for ErrorTestLayer {
        fn configure_client(
            &self,
            client: jsonrpsee::ws_client::WsClient,
        ) -> jsonrpsee::ws_client::WsClient {
            if self.should_error {
                tracing::error!(
                    "ErrorTestLayer configured to simulate error: {:?}",
                    self.error_type
                );
            }
            client
        }

        fn is_enabled(&self) -> bool {
            true
        }

        fn layer_name(&self) -> &'static str {
            "error-test"
        }
    }

    #[test]
    fn test_layer_error_propagation_structure() {
        // Test that layers can be created and configured without panicking
        let layers: Vec<Box<dyn WsClientLayer>> = vec![
            Box::new(NoOpLayer),
            Box::new(ErrorTestLayer::new(
                false,
                ConfigurationClientError::NotFound,
            )),
            Box::new(TimeoutLayer::new(Duration::from_secs(30))),
        ];

        // Verify layer count and types
        assert_eq!(layers.len(), 3);

        // Test layer names
        assert_eq!(layers[0].layer_name(), "no-op");
        assert_eq!(layers[1].layer_name(), "error-test");
        assert_eq!(layers[2].layer_name(), "timeout");

        // Test layer enabled status
        assert!(!layers[0].is_enabled()); // NoOpLayer is disabled
        assert!(layers[1].is_enabled()); // ErrorTestLayer is enabled
        assert!(layers[2].is_enabled()); // TimeoutLayer is enabled
    }

    #[test]
    fn test_disabled_layer_filtering() {
        // Test that disabled layers are properly filtered out
        let layers: Vec<Box<dyn WsClientLayer>> = vec![
            Box::new(NoOpLayer), // This should be filtered out
            Box::new(TimeoutLayer::new(Duration::from_secs(30))),
        ];

        let enabled_layers: Vec<_> = layers
            .into_iter()
            .filter(|layer| layer.is_enabled())
            .collect();
        assert_eq!(enabled_layers.len(), 1);
        assert_eq!(enabled_layers[0].layer_name(), "timeout");
    }

    #[test]
    fn test_zero_timeout_layer_disabled() {
        // Test that timeout layer with zero timeout is disabled
        let timeout_layer = TimeoutLayer::new(Duration::ZERO);
        assert!(!timeout_layer.is_enabled());
    }

    #[test]
    fn test_single_attempt_retry_layer_disabled() {
        // Test that retry layer with max_attempts = 1 is disabled
        let retry_policy = RetryPolicy::builder().max_attempts(1).build();
        let retry_layer = RetryLayer::new(retry_policy);
        assert!(!retry_layer.is_enabled());
    }

    #[test]
    fn test_zero_failure_threshold_circuit_breaker_disabled() {
        // Test that circuit breaker with zero failure threshold is disabled
        let cb_config = CircuitBreakerConfig::builder().failure_threshold(0).build();
        let cb_layer = CircuitBreakerLayer::new(cb_config);
        assert!(!cb_layer.is_enabled());
    }

    #[test]
    fn test_zero_reconnect_attempts_layer_disabled() {
        // Test that reconnection layer with zero max attempts is disabled
        let reconnect_config = ReconnectionConfig::builder()
            .max_reconnect_attempts(Some(0))
            .build();
        let reconnect_layer = ReconnectionLayer::new(reconnect_config);
        assert!(!reconnect_layer.is_enabled());
    }
}
