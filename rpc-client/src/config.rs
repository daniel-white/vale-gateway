use std::time::Duration;

/// Simple configuration for the RPC client
/// Most settings have sensible defaults and don't need to be configured
#[derive(Debug, Clone)]
pub struct RpcClientConfig {
    /// Request timeout (default: 30 seconds)
    pub request_timeout: Duration,

    /// Enable connection monitoring (default: true)
    pub enable_monitoring: bool,

    /// Enable metrics collection (default: true)
    pub enable_metrics: bool,
}

impl RpcClientConfig {
    /// Create a new configuration with sensible defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Enable or disable connection monitoring
    pub fn with_monitoring(mut self, enable: bool) -> Self {
        self.enable_monitoring = enable;
        self
    }

    /// Enable or disable metrics collection
    pub fn with_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }
}

impl Default for RpcClientConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            enable_monitoring: true,
            enable_metrics: true,
        }
    }
}

/// Configuration validation error (kept for compatibility)
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Invalid timeout: {0}")]
    InvalidTimeout(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Invalid internal monitoring configuration: {0}")]
    InvalidInternalMonitoring(String),
}

/// Startup mode for client connections (kept for compatibility)
#[derive(Debug, Clone, PartialEq)]
pub enum StartupMode {
    /// Fail immediately if connection cannot be established
    FailFast,
    /// Try to connect gracefully, continue if it fails
    Graceful,
    /// Connect lazily when first needed
    Lazy,
}

impl Default for StartupMode {
    fn default() -> Self {
        Self::Graceful
    }
}

/// Startup configuration (kept for compatibility)
#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub mode: StartupMode,
    pub log_startup_attempts: bool,
    pub initial_connection_timeout: std::time::Duration,
    pub validate_connectivity: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            mode: StartupMode::default(),
            log_startup_attempts: true,
            initial_connection_timeout: std::time::Duration::from_secs(30),
            validate_connectivity: true,
        }
    }
}

/// Reconnection configuration (kept for compatibility)
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    pub max_reconnect_attempts: u32,
    pub queue_requests_during_reconnection: bool,
    pub max_queued_requests: usize,
    pub enable_lazy_connection: bool,
    pub reconnect_base_delay: std::time::Duration,
    pub reconnect_max_delay: std::time::Duration,
    pub reconnect_backoff_multiplier: f64,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            max_reconnect_attempts: 10,
            queue_requests_during_reconnection: true,
            max_queued_requests: 100,
            enable_lazy_connection: false,
            reconnect_base_delay: std::time::Duration::from_millis(100),
            reconnect_max_delay: std::time::Duration::from_secs(30),
            reconnect_backoff_multiplier: 2.0,
        }
    }
}

impl ReconnectionConfig {
    pub fn builder() -> ReconnectionConfigBuilder {
        ReconnectionConfigBuilder::default()
    }
}

/// Builder for ReconnectionConfig (kept for compatibility)
#[derive(Debug, Default)]
pub struct ReconnectionConfigBuilder {
    max_reconnect_attempts: Option<u32>,
    enable_lazy_connection: Option<bool>,
}

impl ReconnectionConfigBuilder {
    pub fn max_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.max_reconnect_attempts = Some(attempts);
        self
    }

    pub fn enable_lazy_connection(mut self, enable: bool) -> Self {
        self.enable_lazy_connection = Some(enable);
        self
    }

    pub fn build(self) -> ReconnectionConfig {
        let default = ReconnectionConfig::default();
        ReconnectionConfig {
            max_reconnect_attempts: self
                .max_reconnect_attempts
                .unwrap_or(default.max_reconnect_attempts),
            queue_requests_during_reconnection: default.queue_requests_during_reconnection,
            max_queued_requests: default.max_queued_requests,
            enable_lazy_connection: self
                .enable_lazy_connection
                .unwrap_or(default.enable_lazy_connection),
            reconnect_base_delay: default.reconnect_base_delay,
            reconnect_max_delay: default.reconnect_max_delay,
            reconnect_backoff_multiplier: default.reconnect_backoff_multiplier,
        }
    }
}

/// Circuit breaker configuration (kept for compatibility)
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: std::time::Duration,
    pub minimum_throughput: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: std::time::Duration::from_secs(60),
            minimum_throughput: 10,
        }
    }
}

/// Timeout configuration (kept for compatibility)
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub default_timeout: std::time::Duration,
    pub per_operation_timeouts: std::collections::HashMap<String, std::time::Duration>,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout: std::time::Duration::from_secs(30),
            per_operation_timeouts: std::collections::HashMap::new(),
        }
    }
}

/// Retry policy (kept for compatibility)
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl ReconnectionConfig {
    pub fn delay_for_reconnect_attempt(&self, attempt: u32) -> std::time::Duration {
        let delay_ms = self.reconnect_base_delay.as_millis() as u64 * (2_u64.pow(attempt));
        let delay = std::time::Duration::from_millis(delay_ms);
        std::cmp::min(delay, self.reconnect_max_delay)
    }
}
impl RetryPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let delay_ms =
            self.base_delay.as_millis() as f64 * self.backoff_multiplier.powi(attempt as i32);
        let delay = std::time::Duration::from_millis(delay_ms as u64);
        std::cmp::min(delay, self.max_delay)
    }
}
impl RpcClientConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.request_timeout.as_secs() == 0 {
            return Err(ConfigValidationError::InvalidTimeout(
                "Request timeout cannot be zero".to_string(),
            ));
        }
        Ok(())
    }
}
