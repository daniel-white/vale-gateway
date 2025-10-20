use crate::transport::layers::{InternalConnectionMonitor, MonitoringConfig};
use http::Uri;
use opentelemetry::trace::Tracer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use vg_rpc::ConfigurationApiClient;

/// Helper function to create a request context for health checks
fn create_health_check_context() -> vg_rpc::RequestContext {
    // Create a minimal span for health checks using the tracer
    let tracer = opentelemetry::global::tracer("health_check");
    let span = tracer.start("health_check");
    vg_rpc::RequestContext::new(span)
}

/// Manages the lifecycle of internal connection monitoring
/// This manager handles starting, stopping, and coordinating monitoring tasks
#[derive(Debug)]
pub struct MonitoringManager {
    /// Configuration for monitoring behavior
    config: MonitoringConfig,
    /// URI being monitored
    uri: Uri,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
    /// Shutdown signal receiver (for cloning)
    shutdown_rx: watch::Receiver<bool>,
    /// Handle to the monitoring task
    monitor_handle: Option<JoinHandle<()>>,
}

impl MonitoringManager {
    /// Create a new monitoring manager
    pub fn new(config: MonitoringConfig, uri: Uri) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Self {
            config,
            uri,
            shutdown_tx,
            shutdown_rx,
            monitor_handle: None,
        }
    }

    /// Start monitoring with the provided health check function
    pub fn start_monitoring(
        &mut self,
        health_check_fn: Arc<
            dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync,
        >,
    ) -> Result<(), MonitoringError> {
        if self.monitor_handle.is_some() {
            return Err(MonitoringError::AlreadyStarted);
        }

        // Validate configuration before starting
        self.config
            .validate()
            .map_err(MonitoringError::ConfigurationError)?;

        info!(
            target: "rpc_client::monitoring",
            uri = %self.uri,
            "Starting connection monitoring manager"
        );

        // Create and start the internal monitor
        let monitor = InternalConnectionMonitor::new(
            self.config.clone(),
            self.uri.clone(),
            self.shutdown_rx.clone(),
            health_check_fn,
        );

        let handle = monitor.start();
        self.monitor_handle = Some(handle);

        info!(
            target: "rpc_client::monitoring",
            uri = %self.uri,
            "Connection monitoring started successfully"
        );

        Ok(())
    }

    /// Stop monitoring and clean up resources
    pub async fn stop_monitoring(&mut self) -> Result<(), MonitoringError> {
        if let Some(handle) = self.monitor_handle.take() {
            info!(
                target: "rpc_client::monitoring",
                uri = %self.uri,
                "Stopping connection monitoring"
            );

            // Send shutdown signal
            if let Err(e) = self.shutdown_tx.send(true) {
                warn!(
                    target: "rpc_client::monitoring",
                    uri = %self.uri,
                    error = ?e,
                    "Failed to send shutdown signal to monitor"
                );
            }

            // Wait for monitor to stop with timeout
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(())) => {
                    info!(
                        target: "rpc_client::monitoring",
                        uri = %self.uri,
                        "Connection monitoring stopped successfully"
                    );
                }
                Ok(Err(e)) => {
                    error!(
                        target: "rpc_client::monitoring",
                        uri = %self.uri,
                        error = ?e,
                        "Monitor task panicked during shutdown"
                    );
                    return Err(MonitoringError::ShutdownError(
                        "Monitor task panicked".to_string(),
                    ));
                }
                Err(_) => {
                    error!(
                        target: "rpc_client::monitoring",
                        uri = %self.uri,
                        "Monitor shutdown timed out"
                    );
                    return Err(MonitoringError::ShutdownError(
                        "Shutdown timeout".to_string(),
                    ));
                }
            }

            // Reset shutdown signal for potential restart
            if let Err(e) = self.shutdown_tx.send(false) {
                warn!(
                    target: "rpc_client::monitoring",
                    uri = %self.uri,
                    error = ?e,
                    "Failed to reset shutdown signal"
                );
            }
        }

        Ok(())
    }

    /// Check if monitoring is currently active
    pub fn is_monitoring(&self) -> bool {
        self.monitor_handle.is_some() && !self.monitor_handle.as_ref().unwrap().is_finished()
    }

    /// Get the current monitoring configuration
    pub fn config(&self) -> &MonitoringConfig {
        &self.config
    }

    /// Update monitoring configuration (requires restart to take effect)
    pub fn update_config(&mut self, new_config: MonitoringConfig) -> Result<(), MonitoringError> {
        new_config
            .validate()
            .map_err(MonitoringError::ConfigurationError)?;

        let was_monitoring = self.is_monitoring();
        self.config = new_config;

        if was_monitoring {
            warn!(
                target: "rpc_client::monitoring",
                uri = %self.uri,
                "Monitoring configuration updated - restart monitoring to apply changes"
            );
        }

        Ok(())
    }

    /// Restart monitoring with current configuration
    pub async fn restart_monitoring(
        &mut self,
        health_check_fn: Arc<
            dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync,
        >,
    ) -> Result<(), MonitoringError> {
        info!(
            target: "rpc_client::monitoring",
            uri = %self.uri,
            "Restarting connection monitoring"
        );

        // Stop current monitoring if active
        if self.is_monitoring() {
            self.stop_monitoring().await?;
        }

        // Start monitoring with new configuration
        self.start_monitoring(health_check_fn)?;

        Ok(())
    }

    /// Get monitoring status information
    pub fn status(&self) -> MonitoringStatus {
        MonitoringStatus {
            is_active: self.is_monitoring(),
            uri: self.uri.clone(),
            config: self.config.clone(),
        }
    }
}

impl Drop for MonitoringManager {
    fn drop(&mut self) {
        if self.is_monitoring() {
            warn!(
                target: "rpc_client::monitoring",
                uri = %self.uri,
                "MonitoringManager dropped while monitoring is active - sending shutdown signal"
            );

            // Send shutdown signal (best effort)
            let _ = self.shutdown_tx.send(true);
        }
    }
}

/// Status information for monitoring
#[derive(Debug, Clone)]
pub struct MonitoringStatus {
    /// Whether monitoring is currently active
    pub is_active: bool,
    /// URI being monitored
    pub uri: Uri,
    /// Current monitoring configuration
    pub config: MonitoringConfig,
}

/// Errors that can occur during monitoring management
#[derive(Debug, thiserror::Error)]
pub enum MonitoringError {
    #[error("Monitoring is already started")]
    AlreadyStarted,

    #[error("Configuration error: {0}")]
    ConfigurationError(#[from] crate::transport::layers::MonitoringConfigError),

    #[error("Shutdown error: {0}")]
    ShutdownError(String),

    #[error("Health check function error: {0}")]
    HealthCheckError(String),
}

/// Factory for creating health check functions for different client types
pub struct HealthCheckFactory;

impl HealthCheckFactory {
    /// Create a health check function for the client wrapper
    pub fn for_client_wrapper(
        client: Arc<crate::transport::ClientWrapper>,
        listener_ref: vg_config::http::listener::ListenerRef,
    ) -> Arc<dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync> {
        Arc::new(move || {
            let client = client.clone();
            let listener_ref = listener_ref.clone();

            tokio::spawn(async move {
                let start_time = std::time::Instant::now();

                // Create a simple health check request
                let req = vg_rpc::GetListenerRequest::builder()
                    .context(create_health_check_context())
                    .listener_ref(listener_ref)
                    .build();

                match client.listener(req).await {
                    Ok(_) => Ok(start_time.elapsed()),
                    Err(e) => Err(format!("Health check failed: {:?}", e)),
                }
            })
        })
    }

    /// Create a health check function for a simple WebSocket client
    pub fn for_ws_client(
        client: Arc<jsonrpsee::ws_client::WsClient>,
        listener_ref: vg_config::http::listener::ListenerRef,
    ) -> Arc<dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync> {
        Arc::new(move || {
            let client = client.clone();
            let listener_ref = listener_ref.clone();

            tokio::spawn(async move {
                let start_time = std::time::Instant::now();

                // Create a simple health check request
                let req = vg_rpc::GetListenerRequest::builder()
                    .context(create_health_check_context())
                    .listener_ref(listener_ref)
                    .build();

                match client.listener(req).await {
                    Ok(_) => Ok(start_time.elapsed()),
                    Err(e) => Err(format!("Health check failed: {:?}", e)),
                }
            })
        })
    }

    /// Create a health check function for a layered client
    pub fn for_layered_client(
        client: Arc<crate::transport::layers::LayeredClient>,
        listener_ref: vg_config::http::listener::ListenerRef,
    ) -> Arc<dyn Fn() -> tokio::task::JoinHandle<Result<Duration, String>> + Send + Sync> {
        Arc::new(move || {
            let client = client.clone();
            let listener_ref = listener_ref.clone();

            tokio::spawn(async move {
                let start_time = std::time::Instant::now();

                // Create a simple health check request
                let req = vg_rpc::GetListenerRequest::builder()
                    .context(create_health_check_context())
                    .listener_ref(listener_ref)
                    .build();

                match client.listener(req).await {
                    Ok(_) => Ok(start_time.elapsed()),
                    Err(e) => Err(format!("Health check failed: {:?}", e)),
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_monitoring_manager_creation() {
        let config = MonitoringConfig::default();
        let uri: Uri = "ws://localhost:8080".parse().unwrap();

        let manager = MonitoringManager::new(config.clone(), uri.clone());

        assert!(!manager.is_monitoring());
        assert_eq!(manager.config(), &config);
        assert_eq!(manager.status().uri, uri);
        assert!(!manager.status().is_active);
    }

    #[test]
    fn test_monitoring_config_update() {
        let config = MonitoringConfig::default();
        let uri: Uri = "ws://localhost:8080".parse().unwrap();

        let mut manager = MonitoringManager::new(config, uri);

        let new_config = MonitoringConfig {
            check_interval: Duration::from_secs(60),
            health_check_timeout: Duration::from_secs(10),
            critical_threshold: 5,
            enable_heartbeat_logging: true,
        };

        assert!(manager.update_config(new_config.clone()).is_ok());
        assert_eq!(manager.config(), &new_config);
    }

    #[test]
    fn test_monitoring_config_validation() {
        let config = MonitoringConfig::default();
        let uri: Uri = "ws://localhost:8080".parse().unwrap();

        let mut manager = MonitoringManager::new(config, uri);

        // Invalid config - zero check interval
        let invalid_config = MonitoringConfig {
            check_interval: Duration::ZERO,
            health_check_timeout: Duration::from_secs(5),
            critical_threshold: 10,
            enable_heartbeat_logging: false,
        };

        assert!(manager.update_config(invalid_config).is_err());
    }

    #[test]
    fn test_monitoring_status() {
        let config = MonitoringConfig::development();
        let uri: Uri = "ws://localhost:8080".parse().unwrap();

        let manager = MonitoringManager::new(config.clone(), uri.clone());
        let status = manager.status();

        assert!(!status.is_active);
        assert_eq!(status.uri, uri);
        assert_eq!(status.config, config);
    }
}
