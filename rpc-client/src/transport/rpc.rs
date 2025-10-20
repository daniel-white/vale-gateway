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

        let transport = Self::builder()
            .client(Arc::new(layered_client))
            .config(config)
            .connection_handle(connection_handle)
            .event_sender(event_sender)
            .monitoring_manager(monitoring_manager)
            .build();

        // Start connection monitoring using existing MonitoringManager
        transport.start_connection_monitoring().await?;

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

        // Create health check function using existing HealthCheckFactory
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
