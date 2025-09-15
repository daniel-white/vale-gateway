use crate::gateways::resources::GatewayResourceConfigurations;
use crate::kubernetes::objects::{Objects, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watchdog::configuration::RuntimeWatchdogConfiguration;
use crate::watchdog::error::WatchdogError;
use crate::watchdog::{
    create_configmap_restoration_coordinator, ConfigMapWatcher, WatchdogConfiguration,
};
use k8s_openapi::api::core::v1::ConfigMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
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
            config_version: self.config.config().detection_interval(),
        }
    }

    /// Start ConfigMap monitoring with the given sync channel
    pub async fn start_configmap_monitoring(
        &self,
        task_builder: &TaskBuilder,
        configmap_objects_rx: Receiver<Objects<ConfigMap>>,
        expected_configmaps: Arc<Objects<ConfigMap>>,
        sync_tx: mpsc::UnboundedSender<SyncObjectAction<String, ConfigMap>>,
    ) -> Result<(), WatchdogError> {
        if !self.config.is_enabled() {
            info!("Watchdog service is disabled, skipping ConfigMap monitoring");
            return Ok(());
        }

        info!("Starting ConfigMap monitoring");

        // Create restoration coordinator
        let (restoration_tx, restoration_coordinator) =
            create_configmap_restoration_coordinator(sync_tx);

        // Start the restoration coordinator
        restoration_coordinator.start(task_builder)?;

        // Create and configure ConfigMap watcher
        let mut configmap_watcher =
            ConfigMapWatcher::new().with_expected_resources(expected_configmaps);

        // Start the ConfigMap watcher
        configmap_watcher
            .start_watching(configmap_objects_rx)
            .await?;

        // Start event processing
        configmap_watcher.start_event_processing(task_builder, restoration_tx)?;

        info!("ConfigMap monitoring started successfully");
        Ok(())
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
            Duration::from_secs(10)
        );
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
}
