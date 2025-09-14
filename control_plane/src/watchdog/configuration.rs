use crate::watchdog::RetryPolicy;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use typed_builder::TypedBuilder;

/// Configuration for the watchdog service
#[derive(Debug, Clone, Serialize, Deserialize, Getters, CopyGetters, TypedBuilder)]
pub struct WatchdogConfiguration {
    /// Whether the watchdog service is enabled
    #[getset(get_copy = "pub")]
    #[builder(default = true)]
    enabled: bool,

    /// Whether the watchdog is in maintenance mode (suspended operations)
    #[getset(get_copy = "pub")]
    #[builder(default = false)]
    maintenance_mode: bool,

    /// How often to check for configuration drift
    #[getset(get_copy = "pub")]
    #[builder(default_code = "Duration::from_secs(5)")]
    detection_interval: Duration,

    /// Maximum time to wait for a restoration operation
    #[getset(get_copy = "pub")]
    #[builder(default_code = "Duration::from_secs(30)")]
    restoration_timeout: Duration,

    /// Retry policy for failed restoration operations
    #[getset(get = "pub")]
    #[builder(default)]
    retry_policy: RetryPolicy,

    /// Whether to log all drift detection events (can be verbose)
    #[getset(get_copy = "pub", get_mut = "pub")]
    #[builder(default = true)]
    log_drift_events: bool,

    /// Whether to log all restoration events
    #[getset(get_copy = "pub", get_mut = "pub")]
    #[builder(default = true)]
    log_restoration_events: bool,

    /// Whether to log critical alerts for repeated failures
    #[getset(get_copy = "pub", get_mut = "pub")]
    #[builder(default = true)]
    log_critical_alerts: bool,

    /// Maximum number of concurrent restoration operations
    #[getset(get_copy = "pub", get_mut = "pub")]
    #[builder(default = 10)]
    max_concurrent_restorations: usize,

    /// Labels that identify gateway-managed resources
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default_code = r#"vec![
        "app.kubernetes.io/managed-by=vale-gateway-controller".to_string(),
        "app.kubernetes.io/component=gateway".to_string(),
    ]"#)]
    managed_resource_labels: Vec<String>,
}

impl Default for WatchdogConfiguration {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl WatchdogConfiguration {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the watchdog should be active (enabled and not in maintenance mode)
    pub fn is_active(&self) -> bool {
        self.enabled() && !self.maintenance_mode()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.detection_interval().is_zero() {
            return Err("Detection interval must be greater than zero".to_string());
        }

        if self.restoration_timeout().is_zero() {
            return Err("Restoration timeout must be greater than zero".to_string());
        }

        if self.max_concurrent_restorations() == 0 {
            return Err("Max concurrent restorations must be greater than zero".to_string());
        }

        if self.retry_policy().max_attempts() == 0 {
            return Err("Retry policy max attempts must be greater than zero".to_string());
        }

        if self.managed_resource_labels().is_empty() {
            return Err("At least one managed resource label must be specified".to_string());
        }

        Ok(())
    }

    /// Create a configuration for testing with shorter timeouts
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self::builder()
            .detection_interval(Duration::from_millis(100))
            .restoration_timeout(Duration::from_secs(1))
            .retry_policy(
                RetryPolicy::builder()
                    .max_attempts(2)
                    .base_delay(Duration::from_millis(10))
                    .max_delay(Duration::from_millis(100))
                    .build(),
            )
            .max_concurrent_restorations(2)
            .build()
    }
}

/// Runtime configuration that can be updated without restarting the service
#[derive(Debug)]
pub struct RuntimeWatchdogConfiguration {
    /// The current configuration
    config: Arc<WatchdogConfiguration>,

    /// Atomic flag for maintenance mode (for quick checks)
    maintenance_mode: Arc<AtomicBool>,

    /// Atomic flag for enabled state
    enabled: Arc<AtomicBool>,
}

impl RuntimeWatchdogConfiguration {
    /// Create a new runtime configuration
    pub fn new(config: WatchdogConfiguration) -> Self {
        let maintenance_mode = Arc::new(AtomicBool::new(config.maintenance_mode()));
        let enabled = Arc::new(AtomicBool::new(config.enabled()));

        Self {
            config: Arc::new(config),
            maintenance_mode,
            enabled,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> Arc<WatchdogConfiguration> {
        self.config.clone()
    }

    /// Update the configuration
    pub fn update_config(&mut self, new_config: WatchdogConfiguration) -> Result<(), String> {
        new_config.validate()?;

        self.maintenance_mode
            .store(new_config.maintenance_mode(), Ordering::Relaxed);
        self.enabled.store(new_config.enabled(), Ordering::Relaxed);
        self.config = Arc::new(new_config);

        Ok(())
    }

    /// Check if the watchdog is currently active (fast atomic check)
    pub fn is_active(&self) -> bool {
        self.enabled.load(Ordering::Relaxed) && !self.maintenance_mode.load(Ordering::Relaxed)
    }

    /// Set maintenance mode
    pub fn set_maintenance_mode(&self, enabled: bool) {
        self.maintenance_mode.store(enabled, Ordering::Relaxed);
    }

    /// Check if in maintenance mode (fast atomic check)
    pub fn is_maintenance_mode(&self) -> bool {
        self.maintenance_mode.load(Ordering::Relaxed)
    }

    /// Set enabled state
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if enabled (fast atomic check)
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

impl Clone for RuntimeWatchdogConfiguration {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            maintenance_mode: Arc::new(AtomicBool::new(
                self.maintenance_mode.load(Ordering::Relaxed),
            )),
            enabled: Arc::new(AtomicBool::new(self.enabled.load(Ordering::Relaxed))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchdog_configuration_default() {
        let config = WatchdogConfiguration::default();
        assert!(config.enabled());
        assert!(!config.maintenance_mode());
        assert!(config.is_active());
        assert_eq!(config.detection_interval(), Duration::from_secs(5));
        assert_eq!(config.restoration_timeout(), Duration::from_secs(30));
        assert_eq!(config.retry_policy().max_attempts(), 3);
        assert!(!config.managed_resource_labels().is_empty());
    }

    #[test]
    fn test_watchdog_configuration_validation() {
        let mut config = WatchdogConfiguration::default();
        assert!(config.validate().is_ok());

        // Test invalid detection interval
        *config.detection_interval_mut() = Duration::from_secs(0);
        assert!(config.validate().is_err());

        // Reset and test invalid restoration timeout
        config = WatchdogConfiguration::default();
        *config.restoration_timeout_mut() = Duration::from_secs(0);
        assert!(config.validate().is_err());

        // Reset and test invalid max concurrent restorations
        config = WatchdogConfiguration::default();
        *config.max_concurrent_restorations_mut() = 0;
        assert!(config.validate().is_err());

        // Reset and test empty managed resource labels
        config = WatchdogConfiguration::default();
        config.managed_resource_labels_mut().clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_watchdog_configuration_is_active() {
        let mut config = WatchdogConfiguration::default();
        assert!(config.is_active());

        *config.enabled_mut() = false;
        assert!(!config.is_active());

        *config.enabled_mut() = true;
        *config.maintenance_mode_mut() = true;
        assert!(!config.is_active());
    }

    #[test]
    fn test_watchdog_configuration_for_testing() {
        let config = WatchdogConfiguration::for_testing();
        assert!(config.detection_interval() < Duration::from_secs(1));
        assert!(config.restoration_timeout() < Duration::from_secs(5));
        assert_eq!(config.retry_policy().max_attempts(), 2);
        assert_eq!(config.max_concurrent_restorations(), 2);
    }

    #[test]
    fn test_runtime_watchdog_configuration() {
        let config = WatchdogConfiguration::default();
        let runtime_config = RuntimeWatchdogConfiguration::new(config);

        assert!(runtime_config.is_active());
        assert!(runtime_config.is_enabled());
        assert!(!runtime_config.is_maintenance_mode());

        // Test maintenance mode toggle
        runtime_config.set_maintenance_mode(true);
        assert!(!runtime_config.is_active());
        assert!(runtime_config.is_maintenance_mode());

        runtime_config.set_maintenance_mode(false);
        assert!(runtime_config.is_active());

        // Test enabled toggle
        runtime_config.set_enabled(false);
        assert!(!runtime_config.is_active());
        assert!(!runtime_config.is_enabled());
    }

    #[test]
    fn test_runtime_configuration_update() {
        let initial_config = WatchdogConfiguration::default();
        let mut runtime_config = RuntimeWatchdogConfiguration::new(initial_config);

        let new_config = WatchdogConfiguration::builder()
            .enabled(false)
            .maintenance_mode(true)
            .detection_interval(Duration::from_secs(10))
            .build();

        assert!(runtime_config.update_config(new_config).is_ok());
        assert!(!runtime_config.is_enabled());
        assert!(runtime_config.is_maintenance_mode());
        assert_eq!(
            runtime_config.config().detection_interval(),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn test_runtime_configuration_update_validation() {
        let initial_config = WatchdogConfiguration::default();
        let mut runtime_config = RuntimeWatchdogConfiguration::new(initial_config);

        let invalid_config = WatchdogConfiguration::builder()
            .detection_interval(Duration::from_secs(0))
            .build();

        assert!(runtime_config.update_config(invalid_config).is_err());
    }

    #[test]
    fn test_runtime_configuration_clone() {
        let config = WatchdogConfiguration::default();
        let runtime_config = RuntimeWatchdogConfiguration::new(config);

        runtime_config.set_maintenance_mode(true);
        runtime_config.set_enabled(false);

        let cloned = runtime_config.clone();
        assert_eq!(
            cloned.is_maintenance_mode(),
            runtime_config.is_maintenance_mode()
        );
        assert_eq!(cloned.is_enabled(), runtime_config.is_enabled());
    }

    #[test]
    fn test_configuration_serialization() {
        let config = WatchdogConfiguration::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WatchdogConfiguration = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enabled(), deserialized.enabled());
        assert_eq!(config.maintenance_mode(), deserialized.maintenance_mode());
        assert_eq!(
            config.detection_interval(),
            deserialized.detection_interval()
        );
    }
}
