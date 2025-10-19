use std::collections::HashMap;
use std::time::Duration;
use typed_builder::TypedBuilder;

/// Configuration for robust RPC client features
#[derive(Debug, Clone, TypedBuilder)]
pub struct RobustClientConfig {
    /// Timeout configuration for requests
    #[builder(default)]
    pub timeout: Option<TimeoutConfig>,

    /// Circuit breaker configuration
    #[builder(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,

    /// Retry policy configuration
    #[builder(default)]
    pub retry: Option<RetryPolicy>,

    /// Reconnection configuration
    #[builder(default)]
    pub reconnection: Option<ReconnectionConfig>,

    /// Instrumentation configuration
    #[builder(default = InstrumentationConfig::default())]
    pub instrumentation: InstrumentationConfig,
}

impl RobustClientConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if let Some(timeout) = &self.timeout {
            timeout.validate()?;
        }

        if let Some(circuit_breaker) = &self.circuit_breaker {
            circuit_breaker.validate()?;
        }

        if let Some(retry) = &self.retry {
            retry.validate()?;
        }

        if let Some(reconnection) = &self.reconnection {
            reconnection.validate()?;
        }

        Ok(())
    }

    /// Create a production-ready configuration with conservative settings
    pub fn production() -> Self {
        Self::builder()
            .timeout(Some(
                TimeoutConfig::builder()
                    .default_timeout(Duration::from_secs(30))
                    .build(),
            ))
            .circuit_breaker(Some(
                CircuitBreakerConfig::builder()
                    .failure_threshold(5)
                    .success_threshold(3)
                    .timeout(Duration::from_secs(60))
                    .minimum_throughput(10)
                    .build(),
            ))
            .retry(Some(
                RetryPolicy::builder()
                    .max_attempts(3)
                    .base_delay(Duration::from_millis(500))
                    .max_delay(Duration::from_secs(30))
                    .backoff_multiplier(2.0)
                    .jitter(0.1)
                    .build(),
            ))
            .reconnection(Some(
                ReconnectionConfig::builder()
                    .enable_lazy_connection(false)
                    .max_reconnect_attempts(Some(10))
                    .reconnect_base_delay(Duration::from_secs(2))
                    .reconnect_max_delay(Duration::from_secs(300))
                    .queue_requests_during_reconnection(true)
                    .max_queued_requests(100)
                    .build(),
            ))
            .instrumentation(InstrumentationConfig::default())
            .build()
    }

    /// Create a development-friendly configuration with faster timeouts
    pub fn development() -> Self {
        Self::builder()
            .timeout(Some(
                TimeoutConfig::builder()
                    .default_timeout(Duration::from_secs(10))
                    .build(),
            ))
            .circuit_breaker(Some(
                CircuitBreakerConfig::builder()
                    .failure_threshold(3)
                    .success_threshold(2)
                    .timeout(Duration::from_secs(30))
                    .minimum_throughput(5)
                    .build(),
            ))
            .retry(Some(
                RetryPolicy::builder()
                    .max_attempts(2)
                    .base_delay(Duration::from_millis(100))
                    .max_delay(Duration::from_secs(5))
                    .backoff_multiplier(1.5)
                    .jitter(0.1)
                    .build(),
            ))
            .reconnection(Some(
                ReconnectionConfig::builder()
                    .enable_lazy_connection(true)
                    .max_reconnect_attempts(Some(5))
                    .reconnect_base_delay(Duration::from_millis(500))
                    .reconnect_max_delay(Duration::from_secs(30))
                    .queue_requests_during_reconnection(true)
                    .max_queued_requests(50)
                    .build(),
            ))
            .instrumentation(InstrumentationConfig::default())
            .build()
    }
}

impl Default for RobustClientConfig {
    fn default() -> Self {
        Self::builder()
            .timeout(Some(TimeoutConfig::default()))
            .circuit_breaker(Some(CircuitBreakerConfig::default()))
            .retry(Some(RetryPolicy::default()))
            .reconnection(Some(ReconnectionConfig::default()))
            .instrumentation(InstrumentationConfig::default())
            .build()
    }
}

/// Configuration for request timeouts
#[derive(Debug, Clone, TypedBuilder)]
pub struct TimeoutConfig {
    /// Default timeout for all requests
    #[builder(default = Duration::from_secs(30))]
    pub default_timeout: Duration,

    /// Per-operation timeout overrides
    #[builder(default)]
    pub per_operation_timeouts: HashMap<String, Duration>,
}

impl TimeoutConfig {
    /// Validate timeout configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.default_timeout.is_zero() {
            return Err(ConfigValidationError::InvalidTimeout(
                "Default timeout must be positive".to_string(),
            ));
        }

        for (operation, timeout) in &self.per_operation_timeouts {
            if timeout.is_zero() {
                return Err(ConfigValidationError::InvalidTimeout(format!(
                    "Timeout for operation '{}' must be positive",
                    operation
                )));
            }
        }

        Ok(())
    }

    /// Get timeout for a specific operation
    pub fn timeout_for_operation(&self, operation: &str) -> Duration {
        self.per_operation_timeouts
            .get(operation)
            .copied()
            .unwrap_or(self.default_timeout)
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            per_operation_timeouts: HashMap::new(),
        }
    }
}

/// Configuration for circuit breaker
#[derive(Debug, Clone, TypedBuilder)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    #[builder(default = 5)]
    pub failure_threshold: u32,

    /// Number of successes needed to close the circuit from half-open state
    #[builder(default = 3)]
    pub success_threshold: u32,

    /// Timeout before transitioning from open to half-open state
    #[builder(default = Duration::from_secs(60))]
    pub timeout: Duration,

    /// Minimum number of requests before circuit breaker can trip
    #[builder(default = 10)]
    pub minimum_throughput: u32,
}

impl CircuitBreakerConfig {
    /// Validate circuit breaker configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.failure_threshold == 0 {
            return Err(ConfigValidationError::InvalidCircuitBreaker(
                "Failure threshold must be positive".to_string(),
            ));
        }

        if self.success_threshold == 0 {
            return Err(ConfigValidationError::InvalidCircuitBreaker(
                "Success threshold must be positive".to_string(),
            ));
        }

        if self.timeout.is_zero() {
            return Err(ConfigValidationError::InvalidCircuitBreaker(
                "Timeout must be positive".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            minimum_throughput: 10,
        }
    }
}

/// Configuration for retry policy
#[derive(Debug, Clone, TypedBuilder)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    #[builder(default = 3)]
    pub max_attempts: u32,

    /// Base delay between retries
    #[builder(default = Duration::from_millis(100))]
    pub base_delay: Duration,

    /// Maximum delay between retries
    #[builder(default = Duration::from_secs(30))]
    pub max_delay: Duration,

    /// Backoff multiplier for exponential backoff
    #[builder(default = 2.0)]
    pub backoff_multiplier: f64,

    /// Jitter factor to add randomness to backoff (0.0 to 1.0)
    #[builder(default = 0.1)]
    pub jitter: f64,
}

impl RetryPolicy {
    /// Validate retry policy configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.max_attempts == 0 {
            return Err(ConfigValidationError::InvalidRetryPolicy(
                "Max attempts must be positive".to_string(),
            ));
        }

        if self.base_delay.is_zero() {
            return Err(ConfigValidationError::InvalidRetryPolicy(
                "Base delay must be positive".to_string(),
            ));
        }

        if self.max_delay < self.base_delay {
            return Err(ConfigValidationError::InvalidRetryPolicy(
                "Max delay must be >= base delay".to_string(),
            ));
        }

        if self.backoff_multiplier <= 1.0 {
            return Err(ConfigValidationError::InvalidRetryPolicy(
                "Backoff multiplier must be > 1.0".to_string(),
            ));
        }

        if !(0.0..=1.0).contains(&self.jitter) {
            return Err(ConfigValidationError::InvalidRetryPolicy(
                "Jitter must be between 0.0 and 1.0".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate delay for a specific attempt number (0-based)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay_ms = self.base_delay.as_millis() as f64;
        let multiplier = self.backoff_multiplier.powi(attempt as i32 - 1);
        let delay_ms = base_delay_ms * multiplier;

        let max_delay_ms = self.max_delay.as_millis() as f64;
        let capped_delay_ms = delay_ms.min(max_delay_ms);

        // Add jitter
        let jitter_range = capped_delay_ms * self.jitter;
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
        let final_delay_ms = (capped_delay_ms + jitter).max(0.0);

        Duration::from_millis(final_delay_ms as u64)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

/// Configuration for reconnection behavior
#[derive(Debug, Clone, TypedBuilder)]
pub struct ReconnectionConfig {
    /// Enable lazy connection (connect on first request)
    #[builder(default = false)]
    pub enable_lazy_connection: bool,

    /// Maximum number of reconnection attempts (None for unlimited)
    #[builder(default)]
    pub max_reconnect_attempts: Option<u32>,

    /// Base delay between reconnection attempts
    #[builder(default = Duration::from_secs(1))]
    pub reconnect_base_delay: Duration,

    /// Maximum delay between reconnection attempts
    #[builder(default = Duration::from_secs(300))]
    pub reconnect_max_delay: Duration,

    /// Backoff multiplier for reconnection delays
    #[builder(default = 2.0)]
    pub reconnect_backoff_multiplier: f64,

    /// Whether to queue requests during reconnection
    #[builder(default = true)]
    pub queue_requests_during_reconnection: bool,

    /// Maximum number of requests to queue during reconnection
    #[builder(default = 100)]
    pub max_queued_requests: usize,
}

impl ReconnectionConfig {
    /// Validate reconnection configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.reconnect_base_delay.is_zero() {
            return Err(ConfigValidationError::InvalidReconnection(
                "Reconnect base delay must be positive".to_string(),
            ));
        }

        if self.reconnect_max_delay < self.reconnect_base_delay {
            return Err(ConfigValidationError::InvalidReconnection(
                "Reconnect max delay must be >= base delay".to_string(),
            ));
        }

        if self.reconnect_backoff_multiplier <= 1.0 {
            return Err(ConfigValidationError::InvalidReconnection(
                "Reconnect backoff multiplier must be > 1.0".to_string(),
            ));
        }

        if self.max_queued_requests == 0 {
            return Err(ConfigValidationError::InvalidReconnection(
                "Max queued requests must be positive".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate delay for a specific reconnection attempt (0-based)
    pub fn delay_for_reconnect_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return self.reconnect_base_delay;
        }

        let base_delay_ms = self.reconnect_base_delay.as_millis() as f64;
        let multiplier = self.reconnect_backoff_multiplier.powi(attempt as i32);
        let delay_ms = base_delay_ms * multiplier;

        let max_delay_ms = self.reconnect_max_delay.as_millis() as f64;
        let final_delay_ms = delay_ms.min(max_delay_ms);

        Duration::from_millis(final_delay_ms as u64)
    }
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enable_lazy_connection: false,
            max_reconnect_attempts: None,
            reconnect_base_delay: Duration::from_secs(1),
            reconnect_max_delay: Duration::from_secs(300),
            reconnect_backoff_multiplier: 2.0,
            queue_requests_during_reconnection: true,
            max_queued_requests: 100,
        }
    }
}

/// Configuration for instrumentation (metrics and tracing)
#[derive(Debug, Clone, TypedBuilder)]
pub struct InstrumentationConfig {
    /// Enable request metrics
    #[builder(default = true)]
    pub enable_metrics: bool,

    /// Enable distributed tracing
    #[builder(default = true)]
    pub enable_tracing: bool,

    /// Enable detailed logging
    #[builder(default = true)]
    pub enable_logging: bool,

    /// Metrics prefix for all client metrics
    #[builder(default = "rpc_client".to_string())]
    pub metrics_prefix: String,
}

impl Default for InstrumentationConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            enable_tracing: true,
            enable_logging: true,
            metrics_prefix: "rpc_client".to_string(),
        }
    }
}

/// Errors that can occur during configuration validation
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Invalid timeout configuration: {0}")]
    InvalidTimeout(String),

    #[error("Invalid circuit breaker configuration: {0}")]
    InvalidCircuitBreaker(String),

    #[error("Invalid retry policy configuration: {0}")]
    InvalidRetryPolicy(String),

    #[error("Invalid reconnection configuration: {0}")]
    InvalidReconnection(String),
}
