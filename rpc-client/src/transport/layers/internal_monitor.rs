use crate::transport::layers::ConnectionLogger;
use http::Uri;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Internal connection monitor that performs health checks and tracks connection status
/// This monitor runs independently and provides comprehensive logging for connection health
pub struct InternalConnectionMonitor {
    /// Configuration for monitoring behavior
    config: MonitoringConfig,
    /// Logger for structured connection events
    logger: ConnectionLogger,
    /// URI being monitored
    uri: Uri,
    /// Shutdown signal receiver
    shutdown_rx: watch::Receiver<bool>,
    /// Health check function
    health_check_fn:
        Arc<dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync>,
}

impl InternalConnectionMonitor {
    /// Create a new internal connection monitor
    pub fn new(
        config: MonitoringConfig,
        uri: Uri,
        shutdown_rx: watch::Receiver<bool>,
        health_check_fn: Arc<
            dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync,
        >,
    ) -> Self {
        Self {
            config,
            logger: ConnectionLogger::new(),
            uri,
            shutdown_rx,
            health_check_fn,
        }
    }

    /// Start the monitoring loop in a background task
    pub fn start(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run_monitoring_loop().await;
        })
    }

    /// Run the main monitoring loop
    async fn run_monitoring_loop(&mut self) {
        let mut interval = tokio::time::interval(self.config.check_interval);
        let mut consecutive_failures = 0;
        let mut last_success = Instant::now();
        let mut last_heartbeat = Instant::now();

        info!(
            target: "rpc_client::monitor",
            uri = %self.uri,
            check_interval_ms = self.config.check_interval.as_millis(),
            "Starting internal connection monitoring"
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Check if we should shutdown
                    if *self.shutdown_rx.borrow() {
                        info!(
                            target: "rpc_client::monitor",
                            uri = %self.uri,
                            "Shutting down connection monitor"
                        );
                        break;
                    }

                    // Perform health check
                    match self.perform_health_check().await {
                        Ok(response_time) => {
                            self.handle_health_check_success(
                                response_time,
                                &mut consecutive_failures,
                                &mut last_success,
                            );
                        }
                        Err(error) => {
                            consecutive_failures += 1;
                            self.handle_health_check_failure(
                                consecutive_failures,
                                last_success.elapsed(),
                                &error,
                            );
                        }
                    }

                    // Log heartbeat if enabled and enough time has passed
                    if self.config.enable_heartbeat_logging {
                        let heartbeat_interval = Duration::from_secs(300); // 5 minutes
                        if last_heartbeat.elapsed() >= heartbeat_interval {
                            self.log_heartbeat(consecutive_failures, last_success.elapsed());
                            last_heartbeat = Instant::now();
                        }
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        info!(
                            target: "rpc_client::monitor",
                            uri = %self.uri,
                            "Received shutdown signal, stopping monitor"
                        );
                        break;
                    }
                }
            }
        }

        info!(
            target: "rpc_client::monitor",
            uri = %self.uri,
            "Connection monitor stopped"
        );
    }

    /// Perform a health check using the provided health check function
    async fn perform_health_check(&self) -> Result<Duration, String> {
        let start_time = Instant::now();

        // Create the health check task
        let health_check_task = (self.health_check_fn)();

        // Apply timeout to the health check
        match tokio::time::timeout(self.config.health_check_timeout, health_check_task).await {
            Ok(task_result) => match task_result {
                Ok(health_result) => match health_result {
                    Ok(response_time) => {
                        debug!(
                            target: "rpc_client::monitor",
                            uri = %self.uri,
                            response_time_ms = response_time.as_millis(),
                            "Health check successful"
                        );
                        Ok(response_time)
                    }
                    Err(error) => {
                        debug!(
                            target: "rpc_client::monitor",
                            uri = %self.uri,
                            error = %error,
                            "Health check failed"
                        );
                        Err(error)
                    }
                },
                Err(join_error) => {
                    let error = format!("Health check task failed: {}", join_error);
                    debug!(
                        target: "rpc_client::monitor",
                        uri = %self.uri,
                        error = %error,
                        "Health check task panicked or was cancelled"
                    );
                    Err(error)
                }
            },
            Err(_) => {
                let elapsed = start_time.elapsed();
                let error = format!("Health check timed out after {:?}", elapsed);
                debug!(
                    target: "rpc_client::monitor",
                    uri = %self.uri,
                    timeout_ms = self.config.health_check_timeout.as_millis(),
                    elapsed_ms = elapsed.as_millis(),
                    "Health check timed out"
                );
                Err(error)
            }
        }
    }

    /// Handle successful health check
    fn handle_health_check_success(
        &self,
        response_time: Duration,
        consecutive_failures: &mut u32,
        last_success: &mut Instant,
    ) {
        if *consecutive_failures > 0 {
            self.logger
                .log_connection_recovered(response_time, *consecutive_failures);
            info!(
                target: "rpc_client::monitor",
                uri = %self.uri,
                response_time_ms = response_time.as_millis(),
                recovered_after_failures = *consecutive_failures,
                "Connection recovered after failures"
            );
        }

        *consecutive_failures = 0;
        *last_success = Instant::now();
    }

    /// Handle failed health check
    fn handle_health_check_failure(
        &self,
        consecutive_failures: u32,
        downtime: Duration,
        error: &str,
    ) {
        if consecutive_failures >= self.config.critical_threshold {
            self.logger
                .log_critical_connection_failure(downtime, consecutive_failures);
            error!(
                target: "rpc_client::monitor",
                uri = %self.uri,
                consecutive_failures = consecutive_failures,
                downtime_ms = downtime.as_millis(),
                error = error,
                "CRITICAL: Connection has been failing for extended period"
            );
        } else {
            self.logger
                .log_connection_failure(consecutive_failures, error);
            warn!(
                target: "rpc_client::monitor",
                uri = %self.uri,
                consecutive_failures = consecutive_failures,
                error = error,
                "Connection health check failed"
            );
        }
    }

    /// Log periodic heartbeat information
    fn log_heartbeat(&self, consecutive_failures: u32, downtime: Duration) {
        if consecutive_failures > 0 {
            warn!(
                target: "rpc_client::monitor",
                uri = %self.uri,
                consecutive_failures = consecutive_failures,
                downtime_ms = downtime.as_millis(),
                "Monitor heartbeat: Connection experiencing issues"
            );
        } else {
            info!(
                target: "rpc_client::monitor",
                uri = %self.uri,
                "Monitor heartbeat: Connection healthy"
            );
        }
    }
}

/// Configuration for internal connection monitoring
#[derive(Debug, Clone, PartialEq)]
pub struct MonitoringConfig {
    /// Interval between health checks
    pub check_interval: Duration,
    /// Timeout for individual health checks
    pub health_check_timeout: Duration,
    /// Number of consecutive failures before marking as critical
    pub critical_threshold: u32,
    /// Whether to log periodic heartbeat messages
    pub enable_heartbeat_logging: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            health_check_timeout: Duration::from_secs(5),
            critical_threshold: 10, // 5 minutes of failures at 30s intervals
            enable_heartbeat_logging: false, // Reduce log noise by default
        }
    }
}

impl MonitoringConfig {
    /// Create a new monitoring configuration with custom settings
    pub fn new(
        check_interval: Duration,
        health_check_timeout: Duration,
        critical_threshold: u32,
        enable_heartbeat_logging: bool,
    ) -> Self {
        Self {
            check_interval,
            health_check_timeout,
            critical_threshold,
            enable_heartbeat_logging,
        }
    }

    /// Create a configuration optimized for production environments
    pub fn production() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            critical_threshold: 10,
            enable_heartbeat_logging: false,
        }
    }

    /// Create a configuration optimized for development environments
    pub fn development() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            health_check_timeout: Duration::from_secs(3),
            critical_threshold: 5,
            enable_heartbeat_logging: true,
        }
    }

    /// Validate the monitoring configuration
    pub fn validate(&self) -> Result<(), MonitoringConfigError> {
        if self.check_interval.is_zero() {
            return Err(MonitoringConfigError::InvalidCheckInterval(
                "Check interval must be positive".to_string(),
            ));
        }

        if self.health_check_timeout.is_zero() {
            return Err(MonitoringConfigError::InvalidHealthCheckTimeout(
                "Health check timeout must be positive".to_string(),
            ));
        }

        if self.health_check_timeout >= self.check_interval {
            return Err(MonitoringConfigError::InvalidConfiguration(
                "Health check timeout should be less than check interval".to_string(),
            ));
        }

        if self.critical_threshold == 0 {
            return Err(MonitoringConfigError::InvalidCriticalThreshold(
                "Critical threshold must be positive".to_string(),
            ));
        }

        Ok(())
    }
}

/// Errors that can occur during monitoring configuration validation
#[derive(Debug, Clone, thiserror::Error)]
pub enum MonitoringConfigError {
    #[error("Invalid check interval: {0}")]
    InvalidCheckInterval(String),

    #[error("Invalid health check timeout: {0}")]
    InvalidHealthCheckTimeout(String),

    #[error("Invalid critical threshold: {0}")]
    InvalidCriticalThreshold(String),

    #[error("Invalid monitoring configuration: {0}")]
    InvalidConfiguration(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_config_default() {
        let config = MonitoringConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert_eq!(config.health_check_timeout, Duration::from_secs(5));
        assert_eq!(config.critical_threshold, 10);
        assert!(!config.enable_heartbeat_logging);
    }

    #[test]
    fn test_monitoring_config_production() {
        let config = MonitoringConfig::production();
        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert_eq!(config.health_check_timeout, Duration::from_secs(5));
        assert_eq!(config.critical_threshold, 10);
        assert!(!config.enable_heartbeat_logging);
    }

    #[test]
    fn test_monitoring_config_development() {
        let config = MonitoringConfig::development();
        assert_eq!(config.check_interval, Duration::from_secs(10));
        assert_eq!(config.health_check_timeout, Duration::from_secs(3));
        assert_eq!(config.critical_threshold, 5);
        assert!(config.enable_heartbeat_logging);
    }

    #[test]
    fn test_monitoring_config_validation_success() {
        let config = MonitoringConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_monitoring_config_validation_zero_check_interval() {
        let config = MonitoringConfig {
            check_interval: Duration::ZERO,
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(MonitoringConfigError::InvalidCheckInterval(_))
        ));
    }

    #[test]
    fn test_monitoring_config_validation_zero_health_check_timeout() {
        let config = MonitoringConfig {
            health_check_timeout: Duration::ZERO,
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(MonitoringConfigError::InvalidHealthCheckTimeout(_))
        ));
    }

    #[test]
    fn test_monitoring_config_validation_timeout_too_long() {
        let config = MonitoringConfig {
            check_interval: Duration::from_secs(5),
            health_check_timeout: Duration::from_secs(10),
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(MonitoringConfigError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn test_monitoring_config_validation_zero_critical_threshold() {
        let config = MonitoringConfig {
            critical_threshold: 0,
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(MonitoringConfigError::InvalidCriticalThreshold(_))
        ));
    }
}
