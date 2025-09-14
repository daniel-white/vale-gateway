use crate::gateways::resources::GatewayResourceConfigurations;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watchdog::configuration::RuntimeWatchdogConfiguration;
use crate::watchdog::{WatchdogConfiguration, WatchdogError};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;

/// Main watchdog service that orchestrates resource monitoring and restoration
#[derive(Debug)]
pub struct ResourceWatchdogService {
    /// Control plane options
    options: Arc<Options>,

    /// Runtime configuration that can be updated
    config: RuntimeWatchdogConfiguration,
}

impl ResourceWatchdogService {
    /// Create a new watchdog service with default configuration
    pub fn new(options: Arc<Options>) -> Self {
        let config = RuntimeWatchdogConfiguration::new(WatchdogConfiguration::default());

        Self { options, config }
    }

    /// Create a new watchdog service with custom configuration
    pub fn with_config(
        options: Arc<Options>,
        config: WatchdogConfiguration,
    ) -> Result<Self, WatchdogError> {
        config.validate().map_err(WatchdogError::configuration)?;

        let runtime_config = RuntimeWatchdogConfiguration::new(config);

        Ok(Self {
            options,
            config: runtime_config,
        })
    }

    /// Get the current configuration
    pub fn config(&self) -> Arc<WatchdogConfiguration> {
        self.config.config()
    }

    /// Update the watchdog configuration
    pub fn update_config(
        &mut self,
        new_config: WatchdogConfiguration,
    ) -> Result<(), WatchdogError> {
        self.config
            .update_config(new_config)
            .map_err(WatchdogError::configuration)?;
        info!("Watchdog configuration updated successfully");
        Ok(())
    }

    /// Set maintenance mode
    pub fn set_maintenance_mode(&self, enabled: bool) {
        self.config.set_maintenance_mode(enabled);
        if enabled {
            warn!("Watchdog maintenance mode enabled - monitoring suspended");
        } else {
            info!("Watchdog maintenance mode disabled - monitoring resumed");
        }
    }

    /// Check if the watchdog is currently active
    pub fn is_active(&self) -> bool {
        self.config.is_active()
    }

    /// Check if in maintenance mode
    pub fn is_maintenance_mode(&self) -> bool {
        self.config.is_maintenance_mode()
    }

    /// Start all resource watchers
    pub fn start_all_watchers(
        &self,
        task_builder: &TaskBuilder,
        kube_client_rx: Receiver<KubeClientCell>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
    ) {
        if !self.config.is_enabled() {
            info!("Watchdog service is disabled, skipping watcher startup");
            return;
        }

        info!("Starting resource watchdog service");

        // Clone configuration for tasks
        let config = self.config.clone();
        let options = self.options.clone();

        // Start the main watchdog coordinator task
        task_builder
            .new_task("watchdog_coordinator")
            .spawn(async move {
                Self::run_coordinator(config, options, kube_client_rx, configurations_rx).await;
            });

        info!("Resource watchdog service started");
    }

    /// Main coordinator task that manages the watchdog lifecycle
    async fn run_coordinator(
        config: RuntimeWatchdogConfiguration,
        _options: Arc<Options>,
        _kube_client_rx: Receiver<KubeClientCell>,
        _configurations_rx: Receiver<GatewayResourceConfigurations>,
    ) {
        info!("Watchdog coordinator started");

        loop {
            if !config.is_active() {
                debug!("Watchdog is inactive, waiting...");
                tokio::time::sleep(config.config().detection_interval()).await;
                continue;
            }

            // TODO: In future tasks, this will:
            // 1. Monitor for configuration changes
            // 2. Start/stop resource watchers as needed
            // 3. Handle maintenance mode transitions
            // 4. Coordinate with sync controllers

            debug!("Watchdog coordinator tick - monitoring active");
            tokio::time::sleep(config.config().detection_interval()).await;
        }
    }

    /// Validate that the service is properly configured
    pub fn validate(&self) -> Result<(), WatchdogError> {
        self.config
            .config()
            .validate()
            .map_err(WatchdogError::configuration)?;

        if self.options.is_none() {
            return Err(WatchdogError::configuration("Options not provided"));
        }

        Ok(())
    }

    /// Get service statistics for monitoring
    pub fn get_stats(&self) -> WatchdogStats {
        WatchdogStats {
            is_active: self.is_active(),
            is_maintenance_mode: self.is_maintenance_mode(),
            config_version: self.config.config().detection_interval(), // Use detection_interval as a simple version indicator
        }
    }
}

/// Statistics about the watchdog service
#[derive(Debug, Clone)]
pub struct WatchdogStats {
    /// Whether the watchdog is currently active
    pub is_active: bool,

    /// Whether the watchdog is in maintenance mode
    pub is_maintenance_mode: bool,

    /// Configuration version indicator
    pub config_version: std::time::Duration,
}

// Extension trait to check if Options is properly initialized
trait OptionsExt {
    fn is_none(&self) -> bool;
}

impl OptionsExt for Arc<Options> {
    fn is_none(&self) -> bool {
        // For now, we assume Options is always valid if we have an Arc to it
        // In the future, this could check for specific required fields
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use std::time::Duration;

    #[test]
    fn test_watchdog_service_creation() {
        let options = Arc::new(Options::default());
        let service = ResourceWatchdogService::new(options);

        assert!(service.is_active());
        assert!(!service.is_maintenance_mode());
        assert!(service.validate().is_ok());
    }

    #[test]
    fn test_watchdog_service_with_custom_config() {
        let options = Arc::new(Options::default());
        let config = WatchdogConfiguration::builder()
            .enabled(false)
            .maintenance_mode(true)
            .detection_interval(Duration::from_secs(10))
            .build();

        let service = ResourceWatchdogService::with_config(options, config).unwrap();

        assert!(!service.is_active());
        assert!(service.is_maintenance_mode());
        assert_eq!(
            service.config().detection_interval(),
            &Duration::from_secs(10)
        );
    }

    #[test]
    fn test_watchdog_service_with_invalid_config() {
        let options = Arc::new(Options::default());
        let config = WatchdogConfiguration::builder()
            .detection_interval(Duration::from_secs(0)) // Invalid
            .build();

        let result = ResourceWatchdogService::with_config(options, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_watchdog_service_maintenance_mode() {
        let options = Arc::new(Options::default());
        let service = ResourceWatchdogService::new(options);

        assert!(service.is_active());

        service.set_maintenance_mode(true);
        assert!(!service.is_active());
        assert!(service.is_maintenance_mode());

        service.set_maintenance_mode(false);
        assert!(service.is_active());
        assert!(!service.is_maintenance_mode());
    }

    #[test]
    fn test_watchdog_service_config_update() {
        let options = Arc::new(Options::default());
        let mut service = ResourceWatchdogService::new(options);

        let new_config = WatchdogConfiguration::builder()
            .detection_interval(Duration::from_secs(15))
            .max_concurrent_restorations(5)
            .build();

        assert!(service.update_config(new_config).is_ok());
        assert_eq!(
            service.config().detection_interval(),
            &Duration::from_secs(15)
        );
        assert_eq!(service.config().max_concurrent_restorations(), &5);
    }

    #[test]
    fn test_watchdog_service_config_update_invalid() {
        let options = Arc::new(Options::default());
        let mut service = ResourceWatchdogService::new(options);

        let invalid_config = WatchdogConfiguration::builder()
            .detection_interval(Duration::from_secs(0)) // Invalid
            .build();

        assert!(service.update_config(invalid_config).is_err());
    }

    #[test]
    fn test_watchdog_service_stats() {
        let options = Arc::new(Options::default());
        let service = ResourceWatchdogService::new(options);

        let stats = service.get_stats();
        assert!(stats.is_active);
        assert!(!stats.is_maintenance_mode);

        service.set_maintenance_mode(true);
        let stats = service.get_stats();
        assert!(!stats.is_active);
        assert!(stats.is_maintenance_mode);
    }

    #[test]
    fn test_watchdog_service_validation() {
        let options = Arc::new(Options::default());
        let service = ResourceWatchdogService::new(options);

        assert!(service.validate().is_ok());
    }

    #[tokio::test]
    async fn test_watchdog_coordinator_inactive() {
        let config = RuntimeWatchdogConfiguration::new(
            WatchdogConfiguration::builder()
                .enabled(false)
                .detection_interval(Duration::from_millis(10))
                .build(),
        );
        let options = Arc::new(Options::default());

        // Create mock receivers (they won't be used in this test)
        let (_, kube_client_rx) = vg_core::sync::signal::signal("test_kube_client");
        let (_, configurations_rx) = vg_core::sync::signal::signal("test_configurations");

        // Start the coordinator in a separate task
        let coordinator_handle = tokio::spawn(async move {
            // Run for a short time to test the inactive path
            tokio::time::timeout(
                Duration::from_millis(50),
                ResourceWatchdogService::run_coordinator(
                    config,
                    options,
                    kube_client_rx,
                    configurations_rx,
                ),
            )
            .await
        });

        // The coordinator should timeout (which is expected for this test)
        let result = coordinator_handle.await;
        assert!(result.is_ok()); // The task completed
        assert!(result.unwrap().is_err()); // But it timed out, which is expected
    }
}
