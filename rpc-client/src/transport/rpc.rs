use std::sync::Arc;
use std::time::Duration;

use getset::Getters;
use http::Uri;
use thiserror::Error;
use typed_builder::TypedBuilder;

use vg_core::sync::{broadcast, handles::Handle};
use vg_rpc::ConfigurationEvent;

/// Connection state notifications for event clients
#[derive(Debug, Clone)]
pub enum ConnectionState {
    /// Connection has been established or re-established
    Connected,
    /// Connection has been lost
    Disconnected,
    /// Connection is being reconnected
    Reconnecting,
}

use crate::transport::layers::{LayeredClient, MonitoringManager};
use crate::{ConfigurationClientInitError, RpcClientConfig};

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

    /// Connection state notification channel
    #[getset(get = "pub")]
    connection_state_sender: broadcast::Sender<ConnectionState>,

    /// Monitoring manager (reused from existing transport)
    #[getset(get = "pub")]
    monitoring_manager: Arc<tokio::sync::Mutex<MonitoringManager>>,
}

impl RpcTransport {
    /// Create a new RPC transport with default configuration
    pub async fn new(address: Uri) -> Result<Self, RpcTransportError> {
        // Try to build client with retry logic for initial connection
        let config = RpcClientConfig::default();
        let layered_client = Self::create_client_with_retry(&address, &config).await?;

        // Create event distribution channel using core broadcast
        let (event_sender, _) = broadcast::channel(1024);

        // Create connection state notification channel
        let (connection_state_sender, _) = broadcast::channel(16);

        // Create connection handle using core handles
        let (connection_handle, _) = vg_core::sync::handles::handles();

        // Create monitoring manager
        let monitoring_config = crate::transport::layers::MonitoringConfig {
            check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            critical_threshold: 10,
            enable_heartbeat_logging: config.enable_monitoring,
        };
        let monitoring_manager = Arc::new(tokio::sync::Mutex::new(MonitoringManager::new(
            monitoring_config,
            address.clone(),
        )));

        let transport_config = RpcTransportConfig {
            address: address.clone(),
            robust_config: config.clone(),
            event_buffer_size: 1024,
        };

        let client_arc = Arc::new(layered_client);
        let transport = Self {
            client: client_arc.clone(),
            client_replacer: Arc::new(tokio::sync::Mutex::new(client_arc)),
            config: transport_config,
            connection_handle,
            event_sender,
            connection_state_sender,
            monitoring_manager,
        };

        // Start connection monitoring if enabled
        if transport.config.robust_config.enable_monitoring {
            transport.start_connection_monitoring().await?;
        }

        // Start automatic reconnection handler
        transport.start_automatic_reconnection();

        // Send initial connected state
        let _ = transport
            .connection_state_sender
            .send(ConnectionState::Connected);

        Ok(transport)
    }

    /// Create a new RPC transport with custom configuration
    pub async fn with_config(
        address: Uri,
        config: RpcClientConfig,
    ) -> Result<Self, RpcTransportError> {
        let layered_client = Self::create_client_with_retry(&address, &config).await?;

        // Create event distribution channel using core broadcast
        let (event_sender, _) = broadcast::channel(1024);

        // Create connection state notification channel
        let (connection_state_sender, _) = broadcast::channel(16);

        // Create connection handle using core handles
        let (connection_handle, _) = vg_core::sync::handles::handles();

        // Create monitoring manager
        let monitoring_config = crate::transport::layers::MonitoringConfig {
            check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            critical_threshold: 10,
            enable_heartbeat_logging: config.enable_monitoring,
        };
        let monitoring_manager = Arc::new(tokio::sync::Mutex::new(MonitoringManager::new(
            monitoring_config,
            address.clone(),
        )));

        let client_arc = Arc::new(layered_client);
        let transport_config = RpcTransportConfig {
            address: address.clone(),
            robust_config: config.clone(),
            event_buffer_size: 1024,
        };

        let transport = Self {
            client: client_arc.clone(),
            client_replacer: Arc::new(tokio::sync::Mutex::new(client_arc)),
            config: transport_config,
            event_sender,
            connection_state_sender: connection_state_sender.clone(),
            connection_handle,
            monitoring_manager,
        };

        // Start connection monitoring if enabled
        if transport.config.robust_config.enable_monitoring {
            transport.start_connection_monitoring().await?;
        }

        // Start automatic reconnection handler
        transport.start_automatic_reconnection();

        // Send initial connected state
        let _ = connection_state_sender.send(ConnectionState::Connected);

        Ok(transport)
    }

    /// Start connection monitoring using existing MonitoringManager
    /// Use core handles for task management and graceful shutdown
    async fn start_connection_monitoring(&self) -> Result<(), RpcTransportError> {
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

    /// Trigger a reconnection attempt (can be called when connection failures are detected)
    pub async fn trigger_reconnection(&self) -> Result<(), RpcTransportError> {
        let address = self.config.address.clone();
        let config = self.config.robust_config.clone();

        tracing::info!(
            "🔄 Triggering reconnection for RPC transport to {}",
            address
        );

        // Notify event clients that reconnection is starting
        let _ = self
            .connection_state_sender
            .send(ConnectionState::Reconnecting);

        // Attempt to create a new connection and replace the broken one
        match Self::create_new_connection(&address, &config).await {
            Ok(new_layered_client) => {
                // Replace the broken connection with the new one
                {
                    let mut client_guard = self.client_replacer.lock().await;
                    *client_guard = Arc::new(new_layered_client);
                }

                tracing::info!(
                    "🚀 RPC transport successfully reconnected to {} - connection replaced",
                    address
                );

                // Notify event clients that connection is restored
                let _ = self
                    .connection_state_sender
                    .send(ConnectionState::Connected);

                Ok(())
            }
            Err(e) => {
                tracing::warn!("Reconnection attempt to {} failed: {:?}", address, e);
                Err(e)
            }
        }
    }

    /// Start automatic reconnection handler
    /// This provides reactive reconnection capability without proactive health checks
    fn start_automatic_reconnection(&self) {
        tracing::info!(
            "🔄 Reactive reconnection handler ready for RPC transport to {} (no proactive health checks)",
            self.config.address
        );

        // No background task - reconnection will be triggered by clients when they detect failures
        // This prevents the reconnection loop we were seeing in the logs
    }

    /// Create a client with retry logic for both initial connection and reconnection
    /// This handles both initial connection failures and reconnection attempts the same way
    async fn create_client_with_retry(
        address: &Uri,
        config: &RpcClientConfig,
    ) -> Result<LayeredClient, RpcTransportError> {
        let mut retry_count = 0;
        let mut delay = std::time::Duration::from_millis(500);
        let max_delay = std::time::Duration::from_secs(30);
        let max_retries = 10; // Reasonable limit for initial connection

        loop {
            retry_count += 1;

            // Try to create the client with simple configuration
            match crate::EnhancedWsClientBuilder::new()
                .with_robust_config(config.clone())
                .build(address.to_string())
                .await
            {
                Ok(client) => {
                    if retry_count > 1 {
                        tracing::info!(
                            "Successfully connected to {} after {} attempts",
                            address,
                            retry_count
                        );
                    }
                    return Ok(client);
                }
                Err(e) => {
                    if retry_count == 1 {
                        tracing::debug!(
                            "Initial connection to {} failed, will retry: {:?}",
                            address,
                            e
                        );
                    } else if retry_count <= max_retries {
                        tracing::debug!(
                            "Connection attempt #{} to {} failed, retrying in {:?}: {:?}",
                            retry_count,
                            address,
                            delay,
                            e
                        );
                    }

                    if retry_count >= max_retries {
                        tracing::warn!(
                            "Failed to connect to {} after {} attempts, giving up: {:?}",
                            address,
                            retry_count,
                            e
                        );
                        return Err(RpcTransportError::InitializationFailed(e));
                    }

                    // Wait before retrying
                    tokio::time::sleep(delay).await;

                    // Exponential backoff
                    delay = std::cmp::min(delay * 2, max_delay);
                }
            }
        }
    }

    /// Create a new connection for reconnection attempts (reuses the same retry logic)
    async fn create_new_connection(
        address: &Uri,
        config: &RpcClientConfig,
    ) -> Result<LayeredClient, RpcTransportError> {
        // Use the same retry logic for reconnection
        Self::create_client_with_retry(address, config).await
    }
}

/// Configuration for RPC transport
#[derive(Debug, Clone)]
pub struct RpcTransportConfig {
    /// Server address for the connection
    pub address: Uri,

    /// RPC client configuration
    pub robust_config: RpcClientConfig,

    /// Event buffer size for the events client
    pub event_buffer_size: usize,
}

impl RpcTransportConfig {
    /// Validate the RPC transport configuration
    pub fn validate(&self) -> Result<(), crate::config::ConfigValidationError> {
        // Basic validation - timeout should be reasonable
        if self.robust_config.request_timeout.as_secs() == 0 {
            return Err(crate::config::ConfigValidationError::InvalidTimeout(
                "Request timeout cannot be zero".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a configuration with custom RPC client config
    pub fn with_config(address: Uri, config: RpcClientConfig) -> Self {
        Self {
            robust_config: config,
            address,
            event_buffer_size: 1024,
        }
    }
}

impl From<crate::transport::ConfigurationTransportOptions> for RpcTransportConfig {
    fn from(options: crate::transport::ConfigurationTransportOptions) -> Self {
        Self {
            robust_config: options.robust_config().cloned().unwrap_or_default(),
            address: options.address().clone(),
            event_buffer_size: 1024,
        }
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

#[cfg(disabled_tests)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_transport_config_creation() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let robust_config = RpcClientConfig::default();

        let config = RpcTransportConfig {
            robust_config: robust_config.clone(),
            address: address.clone(),
            event_buffer_size: 512,
        };

        assert_eq!(config.address, address);
        assert_eq!(config.event_buffer_size, 512);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rpc_transport_config_with_config() {
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let rpc_config = RpcClientConfig::new().with_timeout(std::time::Duration::from_secs(60));
        let config = RpcTransportConfig::with_config(address.clone(), rpc_config);

        assert_eq!(config.address, address);
        assert_eq!(config.event_buffer_size, 1024);
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
            .robust_config(Some(RpcClientConfig::default()))
            .build();

        let config = RpcTransportConfig::from(transport_options);

        assert_eq!(config.address, address);
        assert_eq!(config.event_buffer_size, 1024);
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
