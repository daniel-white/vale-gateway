use http::Uri;
use std::time::Duration;
use tracing::{error, info, warn};

/// Enhanced logging for connection events with structured logging methods
/// Provides consistent, structured logging for all connection-related events
/// to improve observability and troubleshooting capabilities
#[derive(Debug, Clone)]
pub struct ConnectionLogger {
    /// Target for connection-related log messages
    target: &'static str,
}

impl ConnectionLogger {
    /// Create a new connection logger with default target
    pub fn new() -> Self {
        Self {
            target: "rpc_client::connection",
        }
    }

    /// Create a new connection logger with custom target
    pub fn with_target(target: &'static str) -> Self {
        Self { target }
    }

    /// Log when WebSocket connection is lost
    ///
    /// This method logs connection loss events with structured data including
    /// the URI and error details for troubleshooting connectivity issues.
    ///
    /// # Arguments
    /// * `uri` - The URI of the lost connection
    /// * `error` - Error message describing the connection loss
    pub fn log_connection_lost(&self, uri: &Uri, error: &str) {
        error!(
            target: "rpc_client::connection",
            uri = %uri,
            error = error,
            event = "connection_lost",
            "WebSocket connection lost"
        );
    }

    /// Log reconnection attempt with attempt number and delay
    ///
    /// This method logs each reconnection attempt with structured data
    /// to track reconnection patterns and troubleshoot connectivity issues.
    ///
    /// # Arguments
    /// * `attempt` - The current attempt number (1-based)
    /// * `delay` - The delay before this attempt
    /// * `uri` - The URI being reconnected to
    pub fn log_reconnection_attempt(&self, attempt: u32, delay: Duration, uri: &Uri) {
        warn!(
            target: "rpc_client::connection",
            attempt = attempt,
            delay_ms = delay.as_millis(),
            uri = %uri,
            event = "reconnection_attempt",
            "Attempting to reconnect to configuration service"
        );
    }

    /// Log successful reconnection with connection duration
    ///
    /// This method logs successful reconnection events with timing information
    /// to track connection stability and performance.
    ///
    /// # Arguments
    /// * `uri` - The URI that was successfully reconnected to
    /// * `duration` - How long the reconnection process took
    pub fn log_reconnection_success(&self, uri: &Uri, duration: Duration) {
        info!(
            target: "rpc_client::connection",
            uri = %uri,
            connection_duration_ms = duration.as_millis(),
            event = "reconnection_success",
            "Successfully reconnected to configuration service"
        );
    }

    /// Log when maximum reconnection attempts are reached
    ///
    /// This method logs critical failures when all reconnection attempts
    /// have been exhausted, indicating a persistent connectivity issue.
    ///
    /// # Arguments
    /// * `attempts` - The total number of attempts made
    /// * `uri` - The URI that could not be reconnected to
    pub fn log_reconnection_exhausted(&self, attempts: u32, uri: &Uri) {
        error!(
            target: "rpc_client::connection",
            attempts = attempts,
            uri = %uri,
            event = "reconnection_exhausted",
            "Maximum reconnection attempts reached, giving up"
        );
    }

    /// Log startup connection failure (non-critical for graceful startup)
    ///
    /// This method logs initial connection failures during startup with
    /// appropriate log level based on startup mode. For graceful startup,
    /// this is logged as a warning since the client will retry in background.
    ///
    /// # Arguments
    /// * `uri` - The URI that failed to connect during startup
    /// * `error` - Error message describing the connection failure
    pub fn log_startup_connection_failed(&self, uri: &Uri, error: &str) {
        warn!(
            target: "rpc_client::startup",
            uri = %uri,
            error = error,
            event = "startup_connection_failed",
            "Initial connection failed during startup (will retry in background)"
        );
    }

    /// Log successful startup connection
    ///
    /// This method logs successful initial connections during startup
    /// to confirm that the client is ready to serve requests.
    ///
    /// # Arguments
    /// * `uri` - The URI that was successfully connected to during startup
    pub fn log_startup_connection_success(&self, uri: &Uri) {
        info!(
            target: "rpc_client::startup",
            uri = %uri,
            event = "startup_connection_success",
            "Successfully connected to configuration service during startup"
        );
    }

    /// Log when startup connection validation begins
    ///
    /// This method logs the start of connection validation during startup
    /// to track startup timing and behavior.
    ///
    /// # Arguments
    /// * `uri` - The URI being validated
    /// * `timeout` - The timeout for the validation attempt
    pub fn log_startup_validation_begin(&self, uri: &Uri, timeout: Duration) {
        info!(
            target: "rpc_client::startup",
            uri = %uri,
            timeout_ms = timeout.as_millis(),
            event = "startup_validation_begin",
            "Beginning startup connection validation"
        );
    }

    /// Log when startup enters graceful mode (background connection)
    ///
    /// This method logs when the client enters graceful startup mode,
    /// meaning it will continue startup without blocking on connection.
    ///
    /// # Arguments
    /// * `uri` - The URI that will be connected to in background
    pub fn log_graceful_startup_mode(&self, uri: &Uri) {
        info!(
            target: "rpc_client::startup",
            uri = %uri,
            event = "graceful_startup_mode",
            "Entering graceful startup mode, connection will be established in background"
        );
    }

    /// Log when lazy startup mode is activated
    ///
    /// This method logs when the client is configured for lazy startup,
    /// meaning connection will be deferred until the first request.
    ///
    /// # Arguments
    /// * `uri` - The URI that will be connected to on first request
    pub fn log_lazy_startup_mode(&self, uri: &Uri) {
        info!(
            target: "rpc_client::startup",
            uri = %uri,
            event = "lazy_startup_mode",
            "Lazy startup mode activated, connection will be established on first request"
        );
    }

    /// Log connection state transitions
    ///
    /// This method logs when the connection state changes to help track
    /// connection lifecycle and diagnose state-related issues.
    ///
    /// # Arguments
    /// * `from_state` - The previous connection state
    /// * `to_state` - The new connection state
    /// * `uri` - The URI associated with the connection
    pub fn log_state_transition(&self, from_state: &str, to_state: &str, uri: &Uri) {
        info!(
            target: "rpc_client::connection",
            from_state = from_state,
            to_state = to_state,
            uri = %uri,
            event = "state_transition",
            "Connection state changed"
        );
    }

    /// Log when requests are queued during reconnection
    ///
    /// This method logs when requests are being queued while reconnection
    /// is in progress, helping track request handling during outages.
    ///
    /// # Arguments
    /// * `queue_size` - Current size of the request queue
    /// * `max_queue_size` - Maximum allowed queue size
    pub fn log_request_queued(&self, queue_size: usize, max_queue_size: usize) {
        warn!(
            target: "rpc_client::connection",
            queue_size = queue_size,
            max_queue_size = max_queue_size,
            event = "request_queued",
            "Request queued during reconnection"
        );
    }

    /// Log when queued requests are processed after reconnection
    ///
    /// This method logs when the request queue is being processed after
    /// a successful reconnection, indicating normal operation resumption.
    ///
    /// # Arguments
    /// * `processed_count` - Number of requests processed from queue
    pub fn log_queue_processed(&self, processed_count: usize) {
        info!(
            target: "rpc_client::connection",
            processed_count = processed_count,
            event = "queue_processed",
            "Processing queued requests after reconnection"
        );
    }

    /// Log when request queue is full and requests are being rejected
    ///
    /// This method logs when the request queue reaches capacity and
    /// new requests must be rejected, indicating system overload.
    ///
    /// # Arguments
    /// * `queue_size` - Current (maximum) size of the request queue
    /// * `rejected_count` - Number of requests rejected due to full queue
    pub fn log_queue_full(&self, queue_size: usize, rejected_count: u32) {
        error!(
            target: "rpc_client::connection",
            queue_size = queue_size,
            rejected_count = rejected_count,
            event = "queue_full",
            "Request queue is full, rejecting requests"
        );
    }
}

impl Default for ConnectionLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_connection_logger_creation() {
        let logger = ConnectionLogger::new();
        assert_eq!(logger.target, "rpc_client::connection");

        let custom_logger = ConnectionLogger::with_target("custom::target");
        assert_eq!(custom_logger.target, "custom::target");
    }

    #[test]
    fn test_connection_logger_default() {
        let logger = ConnectionLogger::default();
        assert_eq!(logger.target, "rpc_client::connection");
    }

    #[test]
    fn test_connection_logger_methods_compile() {
        let logger = ConnectionLogger::new();
        let uri: Uri = "ws://localhost:8080".parse().unwrap();

        // Test that all methods compile and can be called
        // In a real test environment, you would capture and verify log output
        logger.log_connection_lost(&uri, "test error");
        logger.log_reconnection_attempt(1, Duration::from_secs(1), &uri);
        logger.log_reconnection_success(&uri, Duration::from_millis(500));
        logger.log_reconnection_exhausted(5, &uri);
        logger.log_startup_connection_failed(&uri, "startup error");
        logger.log_startup_connection_success(&uri);
        logger.log_startup_validation_begin(&uri, Duration::from_secs(5));
        logger.log_graceful_startup_mode(&uri);
        logger.log_lazy_startup_mode(&uri);
        logger.log_state_transition("Disconnected", "Connecting", &uri);
        logger.log_request_queued(5, 10);
        logger.log_queue_processed(3);
        logger.log_queue_full(10, 2);
    }

    #[test]
    fn test_connection_logger_clone() {
        let logger = ConnectionLogger::new();
        let cloned_logger = logger.clone();
        assert_eq!(logger.target, cloned_logger.target);
    }
}
