use crate::{ConfigurationClientError, StartupConfig};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Comprehensive startup logger that integrates with existing ConnectionLogger infrastructure
/// Provides structured logging for all startup phases and outcomes
pub struct StartupLogger {
    span: tracing::Span,
    config: StartupLoggingConfig,
}

/// Configuration for startup logging behavior
#[derive(Debug, Clone, typed_builder::TypedBuilder)]
pub struct StartupLoggingConfig {
    /// Whether to log connection attempts during startup
    #[builder(default = true)]
    pub log_connection_attempts: bool,

    /// Whether to log validation results
    #[builder(default = true)]
    pub log_validation_results: bool,

    /// Whether to log background operations
    #[builder(default = true)]
    pub log_background_operations: bool,

    /// Whether to log startup summary
    #[builder(default = true)]
    pub startup_summary: bool,

    /// Log level for startup events
    #[builder(default = StartupLogLevel::Info)]
    pub log_level: StartupLogLevel,

    /// Whether to include performance metrics in logs
    #[builder(default = true)]
    pub include_performance_metrics: bool,
}

/// Log levels for startup events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for StartupLoggingConfig {
    fn default() -> Self {
        Self {
            log_connection_attempts: true,
            log_validation_results: true,
            log_background_operations: true,
            startup_summary: true,
            log_level: StartupLogLevel::Info,
            include_performance_metrics: true,
        }
    }
}

impl StartupLoggingConfig {
    /// Validate the startup logging configuration
    pub fn validate(&self) -> Result<(), crate::ConfigValidationError> {
        // StartupLoggingConfig is always valid as it only contains boolean flags and enums
        // No validation needed for this configuration
        Ok(())
    }

    /// Create a production-optimized logging configuration
    pub fn production() -> Self {
        Self {
            log_connection_attempts: true,
            log_validation_results: true,
            log_background_operations: false, // Reduce noise in production
            startup_summary: true,
            log_level: StartupLogLevel::Info,
            include_performance_metrics: false, // Reduce overhead
        }
    }

    /// Create a development-friendly logging configuration
    pub fn development() -> Self {
        Self {
            log_connection_attempts: true,
            log_validation_results: true,
            log_background_operations: true,
            startup_summary: true,
            log_level: StartupLogLevel::Debug,
            include_performance_metrics: true,
        }
    }

    /// Create a minimal logging configuration
    pub fn minimal() -> Self {
        Self {
            log_connection_attempts: false,
            log_validation_results: false,
            log_background_operations: false,
            startup_summary: true,
            log_level: StartupLogLevel::Warn,
            include_performance_metrics: false,
        }
    }
}

impl StartupLogger {
    /// Create a new StartupLogger with default configuration
    pub fn new() -> Self {
        Self::with_config(StartupLoggingConfig::default())
    }

    /// Create a new StartupLogger with custom configuration
    pub fn with_config(config: StartupLoggingConfig) -> Self {
        let span = tracing::info_span!(
            "rpc_client_startup",
            startup_mode = tracing::field::Empty,
            client_type = tracing::field::Empty,
            startup_duration_ms = tracing::field::Empty,
        );

        Self { span, config }
    }

    /// Create a StartupLogger from existing ConnectionLogger infrastructure
    pub fn from_connection_logger_config(enable_detailed_logging: bool) -> Self {
        let config = if enable_detailed_logging {
            StartupLoggingConfig::development()
        } else {
            StartupLoggingConfig::production()
        };

        Self::with_config(config)
    }

    /// Log the beginning of startup process
    pub fn log_startup_begin(&self, startup_config: &StartupConfig) {
        if !self.config.log_connection_attempts {
            return;
        }

        let _enter = self.span.enter();

        match self.config.log_level {
            StartupLogLevel::Debug => {
                debug!(
                    target: "rpc_client::startup",
                    mode = ?startup_config.mode,
                    timeout_ms = startup_config.initial_connection_timeout.as_millis(),
                    validate_connectivity = startup_config.validate_connectivity,
                    "Initializing configuration client startup process"
                );
            }
            StartupLogLevel::Info => {
                info!(
                    target: "rpc_client::startup",
                    mode = ?startup_config.mode,
                    timeout_ms = startup_config.initial_connection_timeout.as_millis(),
                    "Initializing configuration client"
                );
            }
            _ => {} // Higher levels don't log startup begin
        }
    }

    /// Log successful startup completion
    pub fn log_startup_success(&self, elapsed: Duration) {
        let _enter = self.span.enter();

        if self.config.include_performance_metrics {
            info!(
                target: "rpc_client::startup",
                elapsed_ms = elapsed.as_millis(),
                status = "success",
                "✓ Configuration client connected successfully during startup"
            );
        } else {
            info!(
                target: "rpc_client::startup",
                status = "success",
                "✓ Configuration client connected successfully"
            );
        }
    }

    /// Log startup fallback to background connection
    pub fn log_startup_fallback(&self, error: &ConfigurationClientError, elapsed: Duration) {
        let _enter = self.span.enter();

        warn!(
            target: "rpc_client::startup",
            error = %error,
            elapsed_ms = elapsed.as_millis(),
            fallback_mode = "background_connection",
            "Configuration service unavailable during startup - client will connect in background"
        );
    }

    /// Log startup timeout
    pub fn log_startup_timeout(&self, elapsed: Duration) {
        let _enter = self.span.enter();

        warn!(
            target: "rpc_client::startup",
            elapsed_ms = elapsed.as_millis(),
            reason = "timeout",
            "Configuration client startup timed out - falling back to background connection"
        );
    }

    /// Log lazy startup mode activation
    pub fn log_lazy_startup(&self, elapsed: Duration) {
        if !self.config.log_connection_attempts {
            return;
        }

        let _enter = self.span.enter();

        info!(
            target: "rpc_client::startup",
            elapsed_ms = elapsed.as_millis(),
            mode = "lazy",
            "Configuration client created in lazy mode - will connect on first request"
        );
    }

    /// Log fail-fast startup begin
    pub fn log_fail_fast_startup_begin(&self) {
        if !self.config.log_connection_attempts {
            return;
        }

        let _enter = self.span.enter();

        info!(
            target: "rpc_client::startup",
            mode = "fail_fast",
            "Attempting fail-fast startup - will fail immediately if connection unavailable"
        );
    }

    /// Log fail-fast startup failure
    pub fn log_fail_fast_startup_failure(
        &self,
        error: &ConfigurationClientError,
        elapsed: Duration,
    ) {
        let _enter = self.span.enter();

        error!(
            target: "rpc_client::startup",
            error = %error,
            elapsed_ms = elapsed.as_millis(),
            mode = "fail_fast",
            "Fail-fast startup failed - configuration service unavailable"
        );
    }

    /// Log connectivity validation results
    pub fn log_connectivity_validation_result(
        &self,
        success: bool,
        response_time: Option<Duration>,
        details: Option<&str>,
    ) {
        if !self.config.log_validation_results {
            return;
        }

        let _enter = self.span.enter();

        if success {
            if let Some(response_time) = response_time {
                info!(
                    target: "rpc_client::startup::validation",
                    response_time_ms = response_time.as_millis(),
                    validation_status = "success",
                    details = details.unwrap_or(""),
                    "✓ Connection validation successful"
                );
            } else {
                info!(
                    target: "rpc_client::startup::validation",
                    validation_status = "success",
                    details = details.unwrap_or(""),
                    "✓ Connection validation successful"
                );
            }
        } else {
            warn!(
                target: "rpc_client::startup::validation",
                validation_status = "failed",
                details = details.unwrap_or(""),
                "✗ Connection validation failed - proceeding with degraded connectivity"
            );
        }
    }

    /// Log background connection establishment start
    pub fn log_background_connection_start(&self) {
        if !self.config.log_background_operations {
            return;
        }

        let _enter = self.span.enter();

        info!(
            target: "rpc_client::startup::background",
            operation = "connection_start",
            "Starting background connection establishment - client will handle requests once connected"
        );
    }

    /// Log background connection establishment progress
    pub fn log_background_connection_progress(&self, attempt: u32, next_retry_in: Duration) {
        if !self.config.log_background_operations {
            return;
        }

        let _enter = self.span.enter();

        debug!(
            target: "rpc_client::startup::background",
            attempt = attempt,
            next_retry_ms = next_retry_in.as_millis(),
            operation = "connection_retry",
            "Background connection attempt failed - will retry"
        );
    }

    /// Log background connection establishment success
    pub fn log_background_connection_success(&self, total_attempts: u32, total_elapsed: Duration) {
        if !self.config.log_background_operations {
            return;
        }

        let _enter = self.span.enter();

        info!(
            target: "rpc_client::startup::background",
            attempts = total_attempts,
            total_elapsed_ms = total_elapsed.as_millis(),
            operation = "connection_success",
            "✓ Background connection established successfully"
        );
    }

    /// Log fallback behavior activation
    pub fn log_fallback_behavior(
        &self,
        fallback_type: &str,
        reason: &str,
        config_details: Option<&str>,
    ) {
        let _enter = self.span.enter();

        info!(
            target: "rpc_client::startup::fallback",
            fallback_type = fallback_type,
            reason = reason,
            config_details = config_details.unwrap_or(""),
            "Activating fallback behavior for startup failure"
        );
    }

    /// Log startup summary with comprehensive metrics
    pub fn log_startup_summary(
        &self,
        success: bool,
        total_elapsed: Duration,
        connection_attempts: u32,
        final_status: &str,
        performance_metrics: Option<StartupPerformanceMetrics>,
    ) {
        if !self.config.startup_summary {
            return;
        }

        let _enter = self.span.enter();

        if self.config.include_performance_metrics && performance_metrics.is_some() {
            let metrics = performance_metrics.unwrap();

            if success {
                info!(
                    target: "rpc_client::startup::summary",
                    success = success,
                    total_elapsed_ms = total_elapsed.as_millis(),
                    connection_attempts = connection_attempts,
                    final_status = final_status,
                    dns_resolution_ms = metrics.dns_resolution_time.as_millis(),
                    tcp_connect_ms = metrics.tcp_connect_time.as_millis(),
                    tls_handshake_ms = metrics.tls_handshake_time.map(|d| d.as_millis()),
                    first_request_ms = metrics.first_request_time.map(|d| d.as_millis()),
                    "🚀 Client startup completed successfully"
                );
            } else {
                warn!(
                    target: "rpc_client::startup::summary",
                    success = success,
                    total_elapsed_ms = total_elapsed.as_millis(),
                    connection_attempts = connection_attempts,
                    final_status = final_status,
                    "⚠️ Client startup completed with fallback behavior"
                );
            }
        } else {
            if success {
                info!(
                    target: "rpc_client::startup::summary",
                    success = success,
                    total_elapsed_ms = total_elapsed.as_millis(),
                    connection_attempts = connection_attempts,
                    final_status = final_status,
                    "🚀 Client startup completed successfully"
                );
            } else {
                warn!(
                    target: "rpc_client::startup::summary",
                    success = success,
                    total_elapsed_ms = total_elapsed.as_millis(),
                    connection_attempts = connection_attempts,
                    final_status = final_status,
                    "⚠️ Client startup completed with fallback behavior"
                );
            }
        }
    }

    /// Log integration with existing ConnectionLogger
    pub fn log_connection_logger_integration(&self, connection_logger_enabled: bool) {
        if !self.config.log_background_operations {
            return;
        }

        let _enter = self.span.enter();

        debug!(
            target: "rpc_client::startup::integration",
            connection_logger_enabled = connection_logger_enabled,
            "Integrating startup logging with existing ConnectionLogger infrastructure"
        );
    }

    /// Create a child span for specific startup operations
    pub fn create_operation_span(&self, operation: &str) -> tracing::Span {
        tracing::info_span!(
            parent: &self.span,
            "startup_operation",
            operation = operation,
            start_time = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
        )
    }

    /// Update the main span with startup results
    pub fn update_span_with_results(&self, success: bool, elapsed: Duration, final_status: &str) {
        self.span.record("startup_duration_ms", elapsed.as_millis());
        self.span.record("startup_success", success);
        self.span.record("final_status", final_status);
    }
}

/// Performance metrics collected during startup
#[derive(Debug, Clone)]
pub struct StartupPerformanceMetrics {
    /// Time spent on DNS resolution
    pub dns_resolution_time: Duration,

    /// Time spent establishing TCP connection
    pub tcp_connect_time: Duration,

    /// Time spent on TLS handshake (if applicable)
    pub tls_handshake_time: Option<Duration>,

    /// Time for first successful request
    pub first_request_time: Option<Duration>,

    /// Memory usage during startup
    pub memory_usage_bytes: Option<u64>,

    /// Number of connection attempts
    pub connection_attempts: u32,
}

impl Default for StartupLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StartupMode;

    #[test]
    fn test_startup_logger_creation() {
        let logger = StartupLogger::new();
        assert!(!logger.span.is_disabled());
    }

    #[test]
    fn test_startup_logger_with_config() {
        let config = StartupLoggingConfig::production();
        let logger = StartupLogger::with_config(config.clone());

        assert_eq!(logger.config.log_level, StartupLogLevel::Info);
        assert!(!logger.config.include_performance_metrics);
    }

    #[test]
    fn test_startup_logging_config_presets() {
        let prod_config = StartupLoggingConfig::production();
        assert!(!prod_config.log_background_operations);
        assert!(!prod_config.include_performance_metrics);

        let dev_config = StartupLoggingConfig::development();
        assert!(dev_config.log_background_operations);
        assert!(dev_config.include_performance_metrics);
        assert_eq!(dev_config.log_level, StartupLogLevel::Debug);

        let minimal_config = StartupLoggingConfig::minimal();
        assert!(!minimal_config.log_connection_attempts);
        assert!(!minimal_config.log_validation_results);
        assert_eq!(minimal_config.log_level, StartupLogLevel::Warn);
    }

    #[test]
    fn test_startup_performance_metrics() {
        let metrics = StartupPerformanceMetrics {
            dns_resolution_time: Duration::from_millis(10),
            tcp_connect_time: Duration::from_millis(50),
            tls_handshake_time: Some(Duration::from_millis(100)),
            first_request_time: Some(Duration::from_millis(200)),
            memory_usage_bytes: Some(1024 * 1024),
            connection_attempts: 1,
        };

        assert_eq!(metrics.dns_resolution_time.as_millis(), 10);
        assert_eq!(metrics.connection_attempts, 1);
    }

    #[tokio::test]
    async fn test_startup_logger_operations() {
        let logger = StartupLogger::new();

        // Test that logging operations don't panic
        let config = crate::StartupConfig::builder()
            .mode(StartupMode::Graceful)
            .build();

        logger.log_startup_begin(&config);
        logger.log_startup_success(Duration::from_millis(100));
        logger.log_connectivity_validation_result(
            true,
            Some(Duration::from_millis(50)),
            Some("test"),
        );

        // Test span creation
        let span = logger.create_operation_span("test_operation");
        assert!(!span.is_disabled());
    }
}
