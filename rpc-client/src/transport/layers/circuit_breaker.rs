use crate::config::CircuitBreakerConfig;
use crate::instrumentation::ClientMetrics;
use jsonrpsee::core::client::Error as JsonRpcError;
use jsonrpsee::ws_client::WsClient;
use opentelemetry::KeyValue;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, warn};

use super::WsClientLayer;

/// Circuit breaker layer that prevents cascading failures by temporarily stopping requests
/// to failing services when failure thresholds are exceeded.
pub struct CircuitBreakerLayer {
    config: CircuitBreakerConfig,
    metrics: Option<Arc<ClientMetrics>>,
    state: Arc<Mutex<CircuitBreakerInternalState>>,
}

impl CircuitBreakerLayer {
    /// Create a new circuit breaker layer with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            metrics: None,
            state: Arc::new(Mutex::new(CircuitBreakerInternalState::new())),
        }
    }

    /// Create a new circuit breaker layer with metrics support
    pub fn with_metrics(config: CircuitBreakerConfig, metrics: Arc<ClientMetrics>) -> Self {
        Self {
            config,
            metrics: Some(metrics),
            state: Arc::new(Mutex::new(CircuitBreakerInternalState::new())),
        }
    }

    /// Check if the circuit breaker should allow a request
    pub fn should_allow_request(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        state.should_allow_request(&self.config)
    }

    /// Record a successful request
    pub fn record_success(&self) {
        let mut state = self.state.lock().unwrap();
        let old_status = state.status;
        state.record_success(&self.config);
        let new_status = state.status;

        if old_status != new_status {
            drop(state); // Release lock before logging
            self.log_state_change(old_status, new_status);
            self.update_metrics(new_status);
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let mut state = self.state.lock().unwrap();
        let old_status = state.status;
        state.record_failure(&self.config);
        let new_status = state.status;

        if old_status != new_status {
            drop(state); // Release lock before logging
            self.log_state_change(old_status, new_status);
            self.update_metrics(new_status);
        }
    }

    /// Record a rejected request (when circuit breaker is open)
    pub fn record_rejection(&self) {
        if let Some(metrics) = &self.metrics {
            // Increment a counter for rejected requests
            metrics.requests_total.add(
                1,
                &[
                    KeyValue::new("status", "rejected"),
                    KeyValue::new("reason", "circuit_breaker_open"),
                ],
            );
        }

        debug!("Request rejected due to circuit breaker being open");
    }

    /// Classify errors to determine if they should trigger circuit breaker
    pub fn classify_error(error: &JsonRpcError) -> bool {
        match error {
            // Network and connection errors should trigger circuit breaker
            JsonRpcError::Transport(_) => true,
            JsonRpcError::RequestTimeout => true,

            // Server errors that indicate service issues
            JsonRpcError::Call(call_error) => {
                // Internal server errors and service unavailable should trigger
                call_error.code() >= -32099 && call_error.code() <= -32000
            }

            // Client errors (bad requests, etc.) should not trigger circuit breaker
            JsonRpcError::InvalidSubscriptionId => false,
            JsonRpcError::InvalidRequestId(_) => false,
            JsonRpcError::ParseError(_) => false,
            JsonRpcError::HttpNotImplemented => false,
            JsonRpcError::EmptyBatchRequest(_) => false,
            JsonRpcError::RestartNeeded(_) => false,
            JsonRpcError::RegisterMethod(_) => false,

            // Custom errors - be conservative and trigger circuit breaker
            JsonRpcError::Custom(_) => true,

            // Service disconnect should trigger circuit breaker
            JsonRpcError::ServiceDisconnect => true,
        }
    }

    /// Update circuit breaker state metrics
    fn update_metrics(&self, state: CircuitBreakerStatus) {
        if let Some(metrics) = &self.metrics {
            let state_value = match state {
                CircuitBreakerStatus::Closed => 0,
                CircuitBreakerStatus::Open => 1,
                CircuitBreakerStatus::HalfOpen => 2,
            };
            metrics.circuit_breaker_state.record(state_value, &[]);
        }
    }

    /// Get current circuit breaker statistics for monitoring
    pub fn get_stats(&self) -> CircuitBreakerStats {
        let state = self.state.lock().unwrap();
        CircuitBreakerStats {
            status: state.status,
            failure_count: state.failure_count,
            success_count: state.success_count,
            request_count: state.request_count,
            last_failure_time: state.last_failure_time,
        }
    }

    /// Log circuit breaker state changes
    fn log_state_change(&self, old_state: CircuitBreakerStatus, new_state: CircuitBreakerStatus) {
        let stats = self.get_stats();

        match (old_state, new_state) {
            (CircuitBreakerStatus::Closed, CircuitBreakerStatus::Open) => {
                warn!(
                    failure_threshold = self.config.failure_threshold,
                    timeout_seconds = self.config.timeout.as_secs(),
                    failure_count = stats.failure_count,
                    request_count = stats.request_count,
                    "Circuit breaker opened due to failure threshold exceeded"
                );
            }
            (CircuitBreakerStatus::Open, CircuitBreakerStatus::HalfOpen) => {
                info!(
                    timeout_seconds = self.config.timeout.as_secs(),
                    failure_count = stats.failure_count,
                    "Circuit breaker transitioned to half-open state after timeout"
                );
            }
            (CircuitBreakerStatus::HalfOpen, CircuitBreakerStatus::Closed) => {
                info!(
                    success_threshold = self.config.success_threshold,
                    success_count = stats.success_count,
                    "Circuit breaker closed after successful recovery"
                );
            }
            (CircuitBreakerStatus::HalfOpen, CircuitBreakerStatus::Open) => {
                warn!(
                    failure_count = stats.failure_count,
                    "Circuit breaker reopened after failed recovery attempt"
                );
            }
            _ => {
                debug!(
                    old_state = ?old_state,
                    new_state = ?new_state,
                    failure_count = stats.failure_count,
                    success_count = stats.success_count,
                    request_count = stats.request_count,
                    "Circuit breaker state change"
                );
            }
        }
    }
}

impl WsClientLayer for CircuitBreakerLayer {
    fn configure_client(&self, client: WsClient) -> WsClient {
        // For now, we return the client as-is since we need to implement
        // the actual circuit breaker wrapping logic in a future iteration.
        // The circuit breaker will be integrated when we implement the
        // service wrapping approach.

        debug!(
            failure_threshold = self.config.failure_threshold,
            success_threshold = self.config.success_threshold,
            timeout_seconds = self.config.timeout.as_secs(),
            minimum_throughput = self.config.minimum_throughput,
            "Circuit breaker layer configured"
        );

        client
    }

    fn is_enabled(&self) -> bool {
        // Circuit breaker is enabled if failure threshold > 0
        self.config.failure_threshold > 0
    }

    fn layer_name(&self) -> &'static str {
        "circuit-breaker"
    }
}

/// Internal state of the circuit breaker
#[derive(Debug)]
struct CircuitBreakerInternalState {
    status: CircuitBreakerStatus,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    request_count: u32,
}

impl CircuitBreakerInternalState {
    fn new() -> Self {
        Self {
            status: CircuitBreakerStatus::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            request_count: 0,
        }
    }

    fn should_allow_request(&mut self, config: &CircuitBreakerConfig) -> bool {
        match self.status {
            CircuitBreakerStatus::Closed => true,
            CircuitBreakerStatus::Open => {
                // Check if timeout has elapsed to transition to half-open
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() >= config.timeout {
                        self.status = CircuitBreakerStatus::HalfOpen;
                        self.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerStatus::HalfOpen => true,
        }
    }

    fn record_success(&mut self, config: &CircuitBreakerConfig) {
        self.request_count += 1;

        match self.status {
            CircuitBreakerStatus::Closed => {
                // Reset failure count on success
                self.failure_count = 0;
            }
            CircuitBreakerStatus::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= config.success_threshold {
                    // Close the circuit
                    self.status = CircuitBreakerStatus::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                    self.last_failure_time = None;
                }
            }
            CircuitBreakerStatus::Open => {
                // Should not happen, but handle gracefully
            }
        }
    }

    fn record_failure(&mut self, config: &CircuitBreakerConfig) {
        self.request_count += 1;
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        match self.status {
            CircuitBreakerStatus::Closed => {
                // Check if we should open the circuit
                if self.request_count >= config.minimum_throughput
                    && self.failure_count >= config.failure_threshold
                {
                    self.status = CircuitBreakerStatus::Open;
                }
            }
            CircuitBreakerStatus::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                self.status = CircuitBreakerStatus::Open;
                self.success_count = 0;
            }
            CircuitBreakerStatus::Open => {
                // Already open, just update counters
            }
        }
    }
}

/// Circuit breaker error type
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError {
    #[error("Circuit breaker is open")]
    Open,
    #[error("Request failed: {message}")]
    RequestFailed { message: String },
}

impl From<JsonRpcError> for CircuitBreakerError {
    fn from(error: JsonRpcError) -> Self {
        CircuitBreakerError::RequestFailed {
            message: error.to_string(),
        }
    }
}

/// Circuit breaker status for monitoring and metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerStatus {
    /// Circuit is closed, requests are allowed through
    Closed,
    /// Circuit is open, requests are rejected immediately
    Open,
    /// Circuit is half-open, limited requests are allowed for testing
    HalfOpen,
}

/// Statistics about the circuit breaker state for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    /// Current status of the circuit breaker
    pub status: CircuitBreakerStatus,
    /// Number of consecutive failures
    pub failure_count: u32,
    /// Number of consecutive successes (in half-open state)
    pub success_count: u32,
    /// Total number of requests processed
    pub request_count: u32,
    /// Time of the last failure
    pub last_failure_time: Option<Instant>,
}

#[cfg(disabled_tests)]
mod tests {
    use super::*;
    use crate::config::CircuitBreakerConfig;
    use jsonrpsee::core::client::Error as JsonRpcError;
    use std::time::Duration;

    #[test]
    fn test_circuit_breaker_layer_creation() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(5)
            .success_threshold(3)
            .timeout(Duration::from_secs(60))
            .minimum_throughput(10)
            .build();

        let layer = CircuitBreakerLayer::new(config);
        assert_eq!(layer.config.failure_threshold, 5);
        assert_eq!(layer.config.success_threshold, 3);
        assert_eq!(layer.config.timeout, Duration::from_secs(60));
        assert_eq!(layer.config.minimum_throughput, 10);
    }

    #[test]
    fn test_error_classification() {
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

        // Client errors should not trigger circuit breaker
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::InvalidSubscriptionId
        ));
        let parse_error =
            serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, "test"));
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::ParseError(parse_error)
        ));
    }

    #[test]
    fn test_circuit_breaker_state_transitions() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(3)
            .success_threshold(2)
            .timeout(Duration::from_millis(100))
            .minimum_throughput(3)
            .build();

        let layer = CircuitBreakerLayer::new(config);

        // Initially should allow requests
        assert!(layer.should_allow_request());

        // Record failures to trigger opening
        layer.record_failure();
        layer.record_failure();
        layer.record_failure();

        // Should now be open and reject requests
        assert!(!layer.should_allow_request());

        // Wait for timeout and check half-open transition
        std::thread::sleep(Duration::from_millis(150));
        assert!(layer.should_allow_request()); // Should transition to half-open

        // Record successes to close
        layer.record_success();
        layer.record_success();

        // Should now be closed
        assert!(layer.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_status_values() {
        assert_ne!(CircuitBreakerStatus::Open, CircuitBreakerStatus::Closed);
        assert_ne!(CircuitBreakerStatus::HalfOpen, CircuitBreakerStatus::Closed);
        assert_ne!(CircuitBreakerStatus::HalfOpen, CircuitBreakerStatus::Open);
    }

    #[test]
    fn test_error_conversions() {
        let json_error = JsonRpcError::RequestTimeout;
        let cb_error: CircuitBreakerError = json_error.into();

        match cb_error {
            CircuitBreakerError::RequestFailed { .. } => {
                // Expected
            }
            _ => panic!("Expected RequestFailed variant"),
        }
    }

    #[test]
    fn test_minimum_throughput_requirement() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(2)
            .success_threshold(1)
            .timeout(Duration::from_secs(1))
            .minimum_throughput(5)
            .build();

        let layer = CircuitBreakerLayer::new(config);

        // Record failures below minimum throughput
        layer.record_failure();
        layer.record_failure();

        // Should still allow requests (below minimum throughput)
        assert!(layer.should_allow_request());

        // Record more failures to exceed minimum throughput
        layer.record_failure();
        layer.record_failure();
        layer.record_failure();

        // Should now be open
        assert!(!layer.should_allow_request());
    }

    #[test]
    fn test_half_open_state_recovery() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(2)
            .success_threshold(3)
            .timeout(Duration::from_millis(50))
            .minimum_throughput(2)
            .build();

        let layer = CircuitBreakerLayer::new(config);

        // Trigger circuit breaker to open
        layer.record_failure();
        layer.record_failure();
        assert!(!layer.should_allow_request());

        // Wait for timeout to transition to half-open
        std::thread::sleep(Duration::from_millis(100));
        assert!(layer.should_allow_request()); // Should be half-open now

        // Record partial successes (not enough to close)
        layer.record_success();
        layer.record_success();
        assert!(layer.should_allow_request()); // Still half-open

        // Record final success to close
        layer.record_success();
        assert!(layer.should_allow_request()); // Should be closed now

        // Verify stats
        let stats = layer.get_stats();
        assert_eq!(stats.status, CircuitBreakerStatus::Closed);
        assert_eq!(stats.failure_count, 0);
    }

    #[test]
    fn test_half_open_state_failure() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(2)
            .success_threshold(2)
            .timeout(Duration::from_millis(50))
            .minimum_throughput(2)
            .build();

        let layer = CircuitBreakerLayer::new(config);

        // Trigger circuit breaker to open
        layer.record_failure();
        layer.record_failure();
        assert!(!layer.should_allow_request());

        // Wait for timeout to transition to half-open
        std::thread::sleep(Duration::from_millis(100));
        assert!(layer.should_allow_request()); // Should be half-open now

        // Record a failure in half-open state (should reopen)
        layer.record_failure();
        assert!(!layer.should_allow_request()); // Should be open again

        // Verify stats
        let stats = layer.get_stats();
        assert_eq!(stats.status, CircuitBreakerStatus::Open);
    }

    #[test]
    fn test_circuit_breaker_stats() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(3)
            .success_threshold(2)
            .timeout(Duration::from_secs(1))
            .minimum_throughput(3)
            .build();

        let layer = CircuitBreakerLayer::new(config);

        // Initial state
        let stats = layer.get_stats();
        assert_eq!(stats.status, CircuitBreakerStatus::Closed);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.success_count, 0);
        assert_eq!(stats.request_count, 0);
        assert!(stats.last_failure_time.is_none());

        // Record some failures
        layer.record_failure();
        layer.record_failure();

        let stats = layer.get_stats();
        assert_eq!(stats.failure_count, 2);
        assert_eq!(stats.request_count, 2);
        assert!(stats.last_failure_time.is_some());

        // Record success
        layer.record_success();

        let stats = layer.get_stats();
        assert_eq!(stats.failure_count, 0); // Reset on success in closed state
        assert_eq!(stats.request_count, 3);
    }

    #[test]
    fn test_error_classification_comprehensive() {
        use jsonrpsee::core::client::Error as JsonRpcError;
        use jsonrpsee::types::ErrorObjectOwned;

        // Transport errors should trigger
        assert!(CircuitBreakerLayer::classify_error(
            &JsonRpcError::Transport(Box::new(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "connection refused"
            )))
        ));

        // Timeout should trigger
        assert!(CircuitBreakerLayer::classify_error(
            &JsonRpcError::RequestTimeout
        ));

        // Service disconnect should trigger
        assert!(CircuitBreakerLayer::classify_error(
            &JsonRpcError::ServiceDisconnect
        ));

        // Server errors should trigger (internal server error range)
        let server_error = ErrorObjectOwned::owned(-32000, "Internal error", None::<()>);
        assert!(CircuitBreakerLayer::classify_error(&JsonRpcError::Call(
            server_error
        )));

        // Client errors should not trigger
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::InvalidSubscriptionId
        ));
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::HttpNotImplemented
        ));

        // Parse errors should not trigger
        let parse_error = serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "parse error",
        ));
        assert!(!CircuitBreakerLayer::classify_error(
            &JsonRpcError::ParseError(parse_error)
        ));

        // Custom errors should trigger (conservative approach)
        assert!(CircuitBreakerLayer::classify_error(&JsonRpcError::Custom(
            "custom error".to_string()
        )));
    }

    #[test]
    fn test_circuit_breaker_with_metrics() {
        use crate::instrumentation::ClientMetrics;

        let config = CircuitBreakerConfig::default();
        let meter = opentelemetry::global::meter("test");
        let metrics = Arc::new(ClientMetrics::new(&meter));
        let layer = CircuitBreakerLayer::with_metrics(config, metrics);

        // Test that metrics integration doesn't break functionality
        assert!(layer.should_allow_request());
        layer.record_success();
        layer.record_failure();
        layer.record_rejection();

        // Should still function normally
        assert!(layer.should_allow_request());
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let config = CircuitBreakerConfig::builder()
            .failure_threshold(10)
            .success_threshold(5)
            .timeout(Duration::from_secs(1))
            .minimum_throughput(10)
            .build();

        let layer = Arc::new(CircuitBreakerLayer::new(config));
        let mut handles = vec![];

        // Spawn multiple threads to test concurrent access
        for i in 0..5 {
            let layer_clone = Arc::clone(&layer);
            let handle = thread::spawn(move || {
                for j in 0..10 {
                    if (i + j) % 2 == 0 {
                        layer_clone.record_success();
                    } else {
                        layer_clone.record_failure();
                    }
                    layer_clone.should_allow_request();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify the circuit breaker is still functional
        let stats = layer.get_stats();
        assert!(stats.request_count > 0);
    }
}
