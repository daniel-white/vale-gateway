use std::sync::Arc;

use getset::Getters;
use http::Uri;
use thiserror::Error;
use typed_builder::TypedBuilder;

use vg_core::sync::{broadcast, handles::Handle};
use vg_rpc::ConfigurationEvent;

use crate::transport::layers::{LayeredClient, MonitoringManager};
use crate::{ConfigurationClientInitError, EnhancedWsClientBuilder, RobustClientConfig};

/// RPC transport that provides a shared WebSocket connection for both configuration and events clients
/// Uses getset for clean field access and typed_builder for ergonomic construction
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct RpcTransport {
    /// Reference to shared LayeredClient (from existing transport)
    #[getset(get = "pub")]
    client: Arc<LayeredClient>,

    /// Mutable reference for connection replacement during reconnection
    /// This is separate from the main client to avoid breaking existing API
    client_replacer: Arc<tokio::sync::Mutex<Arc<LayeredClient>>>,

    /// Connection configuration (reused from existing)
    #[getset(get = "pub")]
    config: RpcTransportConfig,

    /// Connection handle using core utilities
    #[getset(get = "pub")]
    connection_handle: Handle,

    /// Event distribution using core broadcast channels
    #[getset(get = "pub")]
    event_sender: broadcast::Sender<ConfigurationEvent>,

    /// Monitoring manager (reused from existing transport)
    #[getset(get = "pub")]
    monitoring_manager: Arc<tokio::sync::Mutex<MonitoringManager>>,
}

impl RpcTransport {
    /// Create a new RPC transport
    /// Reuses existing EnhancedWsClientBuilder and RobustClientConfig
    pub async fn new(
        address: Uri,
        robust_config: RobustClientConfig,
    ) -> Result<Self, RpcTransportError> {
        // Validate configuration before proceeding
        robust_config.validate().map_err(|e| {
            tracing::error!("Invalid robust client configuration: {:?}", e);
            RpcTransportError::InitializationFailed(ConfigurationClientInitError::WsClientError)
        })?;

        // Reuse existing client building logic from builder.rs
        let layered_client = EnhancedWsClientBuilder::new()
            .with_robust_config(robust_config.clone())
            .build(address.to_string())
            .await
            .map_err(RpcTransportError::InitializationFailed)?;

        // Create event distribution channel using core broadcast
        let (event_sender, _) = broadcast::channel(1024);

        // Create connection handle using core handles
        let (connection_handle, _) = vg_core::sync::handles::handles();

        // Create monitoring manager (reused from existing)
        let monitoring_config = robust_config.internal_monitoring.to_monitoring_config();
        let monitoring_manager = Arc::new(tokio::sync::Mutex::new(MonitoringManager::new(
            monitoring_config,
            address.clone(),
        )));

        let config = RpcTransportConfig::builder()
            .robust_config(robust_config)
            .address(address)
            .event_buffer_size(1024)
            .build();

        let client_arc = Arc::new(layered_client);
        let transport = Self::builder()
            .client(client_arc.clone())
            .client_replacer(Arc::new(tokio::sync::Mutex::new(client_arc)))
            .config(config)
            .connection_handle(connection_handle)
            .event_sender(event_sender)
            .monitoring_manager(monitoring_manager)
            .build();

        // Start connection monitoring using existing MonitoringManager
        transport.start_connection_monitoring().await?;

        // Start automatic reconnection handler
        transport.start_automatic_reconnection();

        Ok(transport)
    }

    /// Create a production-ready RPC transport with conservative settings
    pub async fn production(address: Uri) -> Result<Self, RpcTransportError> {
        Self::new(address, RobustClientConfig::production()).await
    }

    /// Create a development-friendly RPC transport with faster timeouts
    pub async fn development(address: Uri) -> Result<Self, RpcTransportError> {
        Self::new(address, RobustClientConfig::development()).await
    }

    /// Create a minimal RPC transport optimized for performance
    pub async fn minimal(address: Uri) -> Result<Self, RpcTransportError> {
        Self::new(address, RobustClientConfig::minimal()).await
    }

    /// Start connection monitoring using existing MonitoringManager
    /// Use core handles for task management and graceful shutdown
    async fn start_connection_monitoring(&self) -> Result<(), RpcTransportError> {
        if !self.config.robust_config.internal_monitoring.enabled {
            tracing::debug!("Internal monitoring disabled, skipping monitoring startup");
            return Ok(());
        }

        let monitoring_manager = self.monitoring_manager.clone();
        let client = self.client.clone();

        let health_check_fn = crate::transport::layers::HealthCheckFactory::for_layered_client(
            client,
            // Use a default listener ref for health checks - this will be overridden by actual clients
            vg_config::http::listener::ListenerRef::from("health-check".to_string()),
        );

        // Start monitoring using existing MonitoringManager
        let mut manager = monitoring_manager.lock().await;
        manager
            .start_monitoring(health_check_fn)
            .map_err(RpcTransportError::MonitoringError)?;

        tracing::info!(
            uri = %self.config.address,
            "RPC transport connection monitoring started successfully"
        );

        Ok(())
    }

    /// Stop connection monitoring and clean up resources
    pub async fn stop_monitoring(&self) -> Result<(), RpcTransportError> {
        let mut manager = self.monitoring_manager.lock().await;
        manager
            .stop_monitoring()
            .await
            .map_err(RpcTransportError::MonitoringError)?;

        tracing::info!(
            uri = %self.config.address,
            "RPC transport connection monitoring stopped"
        );

        Ok(())
    }

    /// Check if monitoring is currently active
    pub async fn is_monitoring(&self) -> bool {
        let manager = self.monitoring_manager.lock().await;
        manager.is_monitoring()
    }

    /// Get connection status information
    pub async fn monitoring_status(&self) -> crate::transport::layers::MonitoringStatus {
        let manager = self.monitoring_manager.lock().await;
        manager.status()
    }

    /// Get access to the current client for making RPC calls
    /// This method provides access to the most recent connection (after any reconnections)
    pub async fn current_client(&self) -> Arc<LayeredClient> {
        let client_guard = self.client_replacer.lock().await;
        client_guard.clone()
    }

    /// Start automatic reconnection handler
    /// This runs in the background and automatically reconnects when the connection is lost
    fn start_automatic_reconnection(&self) {
        let address = self.config.address.clone();
        let robust_config = self.config.robust_config.clone();
        let _monitoring_manager = self.monitoring_manager.clone();
        let client_replacer = self.client_replacer.clone();

        // Spawn background reconnection task
        tokio::spawn(async move {
            tracing::info!(
                "🔄 Started automatic reconnection handler for RPC transport to {}",
                address
            );

            let mut reconnect_attempts = 0;
            let mut delay = std::time::Duration::from_secs(2);
            let max_delay = std::time::Duration::from_secs(30);
            let mut last_failure_time = std::time::Instant::now();
            let mut consecutive_health_failures = 0;

            loop {
                // Check connection health every 30 seconds (less aggressive)
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                // Test connection health by attempting a quick connection
                // This is more reliable than trying to interpret monitoring status
                let connection_needs_repair = {
                    let test_config = RobustClientConfig::minimal();
                    let test_result = tokio::time::timeout(
                        std::time::Duration::from_millis(5000), // Longer timeout to avoid false positives
                        EnhancedWsClientBuilder::new()
                            .with_robust_config(test_config)
                            .build(address.to_string()),
                    )
                    .await;

                    match test_result {
                        Ok(Ok(_)) => {
                            // Connection test succeeded
                            if consecutive_health_failures > 0 {
                                tracing::debug!(
                                    "RPC transport connection to {} is healthy",
                                    address
                                );
                                consecutive_health_failures = 0;
                            }
                            false
                        }
                        Ok(Err(_)) | Err(_) => {
                            // Connection test failed
                            consecutive_health_failures += 1;
                            tracing::debug!(
                                "RPC transport connection test failed for {} (failure #{})",
                                address,
                                consecutive_health_failures
                            );
                            consecutive_health_failures >= 3 // Require 3 consecutive failures to avoid false positives
                        }
                    }
                };

                if connection_needs_repair {
                    let now = std::time::Instant::now();
                    // Only attempt reconnection if enough time has passed since last attempt
                    if now.duration_since(last_failure_time) >= delay {
                        reconnect_attempts += 1;
                        last_failure_time = now;

                        tracing::info!(
                            "🔄 RPC transport connection to {} needs repair, attempting reconnection #{}",
                            address,
                            reconnect_attempts
                        );

                        // Attempt to create a new connection and replace the broken one
                        match RpcTransport::create_new_connection(&address, &robust_config).await {
                            Ok(new_layered_client) => {
                                // Replace the broken connection with the new one
                                {
                                    let mut client_guard = client_replacer.lock().await;
                                    *client_guard = Arc::new(new_layered_client);
                                }

                                tracing::info!(
                                    "🚀 RPC transport successfully reconnected to {} after {} attempts - connection replaced",
                                    address,
                                    reconnect_attempts
                                );

                                // Reset counters on successful reconnection
                                reconnect_attempts = 0;
                                consecutive_health_failures = 0;
                                delay = std::time::Duration::from_secs(2);
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Reconnection attempt #{} to {} failed: {:?} - will retry in {:?}",
                                    reconnect_attempts,
                                    address,
                                    e,
                                    delay
                                );

                                // Exponential backoff
                                delay = std::cmp::min(delay * 2, max_delay);
                            }
                        }
                    }
                } else {
                    // Connection is healthy, reset counters
                    if reconnect_attempts > 0 {
                        reconnect_attempts = 0;
                        delay = std::time::Duration::from_secs(2);
                    }
                }
            }
        });

        tracing::info!(
            "🔄 Automatic reconnection enabled for RPC transport to {}",
            self.config.address
        );
    }

    /// Create a new connection for reconnection attempts
    async fn create_new_connection(
        address: &Uri,
        robust_config: &RobustClientConfig,
    ) -> Result<LayeredClient, RpcTransportError> {
        // Create a simplified config for reconnection attempts
        let mut reconnect_config = robust_config.clone();
        reconnect_config.startup.initial_connection_timeout = std::time::Duration::from_secs(5);
        reconnect_config.internal_monitoring.enabled = false; // Avoid recursive monitoring

        EnhancedWsClientBuilder::new()
            .with_robust_config(reconnect_config)
            .build(address.to_string())
            .await
            .map_err(RpcTransportError::InitializationFailed)
    }

    /// Quick health check for a client to detect stale connections
    async fn is_client_healthy(&self, _client: &LayeredClient) -> bool {
        // Try a very quick connection test with minimal timeout
        // This is a lightweight check to see if the client is responsive
        let test_config = RobustClientConfig::minimal();
        let test_result = tokio::time::timeout(
            std::time::Duration::from_millis(500), // Very short timeout
            EnhancedWsClientBuilder::new()
                .with_robust_config(test_config)
                .build(self.config.address.to_string()),
        )
        .await;

        match test_result {
            Ok(Ok(_)) => {
                tracing::trace!("Client health check passed for {}", self.config.address);
                true
            }
            Ok(Err(_)) | Err(_) => {
                tracing::debug!("Client health check failed for {}", self.config.address);
                false
            }
        }
    }

    /// Trigger an immediate reconnection attempt (non-blocking)
    async fn trigger_immediate_reconnection(&self) {
        let address = self.config.address.clone();
        let robust_config = self.config.robust_config.clone();
        let client_replacer = self.client_replacer.clone();

        tracing::info!("🔄 Triggering immediate reconnection for {}", address);

        // Attempt to create a new connection immediately
        match Self::create_new_connection(&address, &robust_config).await {
            Ok(new_layered_client) => {
                // Replace the broken connection with the new one
                {
                    let mut client_guard = client_replacer.lock().await;
                    *client_guard = Arc::new(new_layered_client);
                }

                tracing::info!(
                    "🚀 Immediate reconnection to {} successful - connection replaced",
                    address
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Immediate reconnection to {} failed: {:?} - background reconnection will continue",
                    address,
                    e
                );
            }
        }
    }
}

/// Configuration for RPC transport
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct RpcTransportConfig {
    /// Reused existing robust configuration
    #[getset(get = "pub")]
    robust_config: RobustClientConfig,

    /// Server address for the connection
    #[getset(get = "pub")]
    address: Uri,

    /// Event buffer size for event distribution
    #[getset(get = "pub")]
    #[builder(default = 1024)]
    event_buffer_size: usize,
}

impl RpcTransportConfig {
    /// Validate the RPC transport configuration
    /// Reuses existing config validation
    pub fn validate(&self) -> Result<(), crate::config::ConfigValidationError> {
        self.robust_config.validate()
    }

    /// Create a production-ready configuration
    pub fn production(address: Uri) -> Self {
        Self::builder()
            .robust_config(RobustClientConfig::production())
            .address(address)
            .event_buffer_size(1024)
            .build()
    }

    /// Create a development-friendly configuration
    pub fn development(address: Uri) -> Self {
        Self::builder()
            .robust_config(RobustClientConfig::development())
            .address(address)
            .event_buffer_size(512)
            .build()
    }

    /// Create a minimal configuration optimized for performance
    pub fn minimal(address: Uri) -> Self {
        Self::builder()
            .robust_config(RobustClientConfig::minimal())
            .address(address)
            .event_buffer_size(256)
            .build()
    }
}

impl From<crate::transport::ConfigurationTransportOptions> for RpcTransportConfig {
    fn from(options: crate::transport::ConfigurationTransportOptions) -> Self {
        Self::builder()
            .robust_config(options.robust_config().cloned().unwrap_or_default())
            .address(options.address().clone())
            .event_buffer_size(1024)
            .build()
    }
}

/// Errors that can occur during RPC transport operations
#[derive(Debug, Error)]
pub enum RpcTransportError {
    /// Reused from existing ConfigurationClientInitError
    #[error("Transport initialization failed")]
    InitializationFailed(#[from] ConfigurationClientInitError),

    /// New error for RPC transport specific issues
    #[error("RPC transport already initialized with different configuration")]
    ConfigurationMismatch,

    /// Reused from existing monitoring errors
    #[error("Monitoring error")]
    MonitoringError(#[from] crate::transport::layers::MonitoringError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_transport_config_creation() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let robust_config = RobustClientConfig::default();

        let config = RpcTransportConfig::builder()
            .robust_config(robust_config.clone())
            .address(address.clone())
            .event_buffer_size(512)
            .build();

        assert_eq!(config.address(), &address);
        assert_eq!(config.event_buffer_size(), &512);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rpc_transport_config_production() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let config = RpcTransportConfig::production(address.clone());

        assert_eq!(config.address(), &address);
        assert_eq!(config.event_buffer_size(), &1024);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rpc_transport_config_development() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let config = RpcTransportConfig::development(address.clone());

        assert_eq!(config.address(), &address);
        assert_eq!(config.event_buffer_size(), &512);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rpc_transport_config_minimal() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let config = RpcTransportConfig::minimal(address.clone());

        assert_eq!(config.address(), &address);
        assert_eq!(config.event_buffer_size(), &256);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rpc_transport_config_from_transport_options() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let listener_ref =
            vg_config::http::listener::ListenerRef::from("test-listener".to_string());

        let transport_options = crate::transport::ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref)
            .address(address.clone())
            .robust_config(Some(RobustClientConfig::default()))
            .build();

        let config = RpcTransportConfig::from(transport_options);

        assert_eq!(config.address(), &address);
        assert_eq!(config.event_buffer_size(), &1024);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rpc_transport_error_display() {
        let init_error =
            RpcTransportError::InitializationFailed(ConfigurationClientInitError::WsClientError);
        assert!(
            init_error
                .to_string()
                .contains("Transport initialization failed")
        );

        let config_error = RpcTransportError::ConfigurationMismatch;
        assert!(
            config_error
                .to_string()
                .contains("already initialized with different configuration")
        );
    }
}
