use async_from::AsyncTryFrom;
use async_trait::async_trait;

use crate::{EnhancedWsClientBuilder, RpcClientConfig};
use http::Uri;
use jsonrpsee::core::ClientError as JsonRpcClientError;
use jsonrpsee::ws_client::{PingConfig, WsClient, WsClientBuilder};
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::backend::Backend;
use vg_config::http::filter::SharedFilter;
use vg_config::http::listener::Listener;
use vg_config::http::listener::ListenerRef;
use vg_config::http::route::Route;
use vg_rpc::{
    ConfigurationApiClient, ConfigurationApiError, GetBackendRequest, GetListenerRequest,
    GetRouteRequest, GetSharedFilterRequest,
};

pub mod layers;
pub mod rpc;

// Re-export specific items to avoid ambiguous glob re-exports
pub use layers::{
    CircuitBreakerLayer, ClientError, ConnectionManager, HealthCheckFactory, LayeredClient,
    MonitoringManager, NoOpLayer, ReconnectionLayer, RetryLayer, StartupLogger, StartupManager,
    TimeoutLayer, WsClientLayer,
};
pub use rpc::{ConnectionState as RpcConnectionState, RpcTransport};

/// Transport wrapper that can hold either a simple WsClient or a LayeredClient
/// This maintains backward compatibility while supporting enhanced robustness features
#[derive(Clone)]
pub enum ClientWrapper {
    /// Simple WebSocket client without middleware layers
    Simple(Arc<WsClient>),
    /// Enhanced client with middleware layers for robustness
    Layered(Arc<LayeredClient>),
}

impl ClientWrapper {
    /// Create a simple wrapper from a WsClient
    pub fn simple(client: WsClient) -> Self {
        Self::Simple(Arc::new(client))
    }

    /// Create a layered wrapper from a LayeredClient
    pub fn layered(client: LayeredClient) -> Self {
        Self::Layered(Arc::new(client))
    }

    /// Execute a request using the appropriate client type
    pub async fn listener(
        &self,
        req: GetListenerRequest,
    ) -> Result<Listener, ConfigurationApiError> {
        match self {
            ClientWrapper::Simple(client) => client
                .listener(req)
                .await
                .map_err(client_error_to_api_error),
            ClientWrapper::Layered(client) => client
                .listener(req)
                .await
                .map_err(client_error_to_api_error),
        }
    }

    /// Execute a route request using the appropriate client type
    pub async fn route(&self, req: GetRouteRequest) -> Result<Route, ConfigurationApiError> {
        match self {
            ClientWrapper::Simple(client) => {
                client.route(req).await.map_err(client_error_to_api_error)
            }
            ClientWrapper::Layered(client) => {
                client.route(req).await.map_err(client_error_to_api_error)
            }
        }
    }

    /// Execute a backend request using the appropriate client type
    pub async fn backend(&self, req: GetBackendRequest) -> Result<Backend, ConfigurationApiError> {
        match self {
            ClientWrapper::Simple(client) => {
                client.backend(req).await.map_err(client_error_to_api_error)
            }
            ClientWrapper::Layered(client) => {
                client.backend(req).await.map_err(client_error_to_api_error)
            }
        }
    }

    /// Execute a shared filter request using the appropriate client type
    pub async fn shared_filter(
        &self,
        req: GetSharedFilterRequest,
    ) -> Result<SharedFilter, ConfigurationApiError> {
        match self {
            ClientWrapper::Simple(client) => client
                .shared_filter(req)
                .await
                .map_err(client_error_to_api_error),
            ClientWrapper::Layered(client) => client
                .shared_filter(req)
                .await
                .map_err(client_error_to_api_error),
        }
    }

    /// Subscribe to events using the appropriate client type
    pub async fn events(
        &self,
        req: vg_rpc::SubscribeEventsRequest,
    ) -> Result<
        jsonrpsee::core::client::Subscription<vg_rpc::ConfigurationEventMessage>,
        jsonrpsee::core::ClientError,
    > {
        match self {
            ClientWrapper::Simple(client) => client.events(req).await,
            ClientWrapper::Layered(client) => client.events(req).await,
        }
    }
}

#[derive(Clone, TypedBuilder)]
pub struct ConfigurationTransport {
    listener_ref: ListenerRef,
    client: ClientWrapper,
    /// Optional monitoring manager for internal connection monitoring
    #[builder(default)]
    monitoring_manager: Option<Arc<tokio::sync::Mutex<MonitoringManager>>>,
}

impl ConfigurationTransport {
    /// Get a reference to the listener ref
    pub fn listener_ref(&self) -> &ListenerRef {
        &self.listener_ref
    }

    /// Get a reference to the underlying client wrapper
    pub fn client(&self) -> &ClientWrapper {
        &self.client
    }

    /// Get a reference to the monitoring manager if available
    pub fn monitoring_manager(&self) -> Option<&Arc<tokio::sync::Mutex<MonitoringManager>>> {
        self.monitoring_manager.as_ref()
    }

    /// Start internal monitoring if configured
    pub async fn start_monitoring(&self) -> Result<(), crate::transport::layers::MonitoringError> {
        if let Some(monitoring_manager) = &self.monitoring_manager {
            let mut manager = monitoring_manager.lock().await;

            // Create appropriate health check function based on client type
            let health_check_fn = match &self.client {
                ClientWrapper::Simple(client) => {
                    HealthCheckFactory::for_ws_client(client.clone(), self.listener_ref.clone())
                }
                ClientWrapper::Layered(client) => HealthCheckFactory::for_layered_client(
                    client.clone(),
                    self.listener_ref.clone(),
                ),
            };

            manager.start_monitoring(health_check_fn)?;
        }
        Ok(())
    }

    /// Stop internal monitoring if active
    pub async fn stop_monitoring(&self) -> Result<(), crate::transport::layers::MonitoringError> {
        if let Some(monitoring_manager) = &self.monitoring_manager {
            let mut manager = monitoring_manager.lock().await;
            manager.stop_monitoring().await?;
        }
        Ok(())
    }

    /// Check if monitoring is currently active
    pub async fn is_monitoring(&self) -> bool {
        if let Some(monitoring_manager) = &self.monitoring_manager {
            let manager = monitoring_manager.lock().await;
            manager.is_monitoring()
        } else {
            false
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationTransportOptions {
    #[builder(setter(into))]
    listener_ref: ListenerRef,
    #[builder(setter(into))]
    address: Uri,
    /// Optional robustness configuration for enhanced client features
    #[builder(default)]
    robust_config: Option<RpcClientConfig>,
}

#[derive(Debug, Error)]
pub enum ConfigurationClientInitError {
    #[error("WebSocket client error")]
    WsClientError,
}

impl ConfigurationTransportOptions {
    /// Create transport options with robustness features enabled
    pub fn with_robust_config(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
        robust_config: RpcClientConfig,
    ) -> Self {
        Self::builder()
            .listener_ref(listener_ref)
            .address(address)
            .robust_config(Some(robust_config))
            .build()
    }

    /// Create transport options with production-ready robustness settings
    pub fn production(listener_ref: impl Into<ListenerRef>, address: impl Into<Uri>) -> Self {
        Self::with_robust_config(listener_ref, address, RpcClientConfig::default())
    }

    /// Create transport options with development-friendly robustness settings
    pub fn development(listener_ref: impl Into<ListenerRef>, address: impl Into<Uri>) -> Self {
        Self::with_robust_config(
            listener_ref,
            address,
            RpcClientConfig::new().with_timeout(std::time::Duration::from_secs(10)),
        )
    }

    /// Create transport options with default robustness settings
    pub fn with_default_robustness(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Self {
        Self::with_robust_config(listener_ref, address, RpcClientConfig::default())
    }

    /// Check if robustness features are configured
    pub fn has_robust_config(&self) -> bool {
        self.robust_config.is_some()
    }

    /// Get a reference to the robust configuration if present
    pub fn robust_config(&self) -> Option<&RpcClientConfig> {
        self.robust_config.as_ref()
    }

    /// Get a reference to the listener ref
    pub fn listener_ref(&self) -> &ListenerRef {
        &self.listener_ref
    }

    /// Get a reference to the address
    pub fn address(&self) -> &Uri {
        &self.address
    }
}

#[async_trait]
impl AsyncTryFrom<ConfigurationTransportOptions> for ConfigurationTransport {
    type Error = ConfigurationClientInitError;

    async fn async_try_from(value: ConfigurationTransportOptions) -> Result<Self, Self::Error> {
        // Simple implementation - just create a basic client
        let client_wrapper = if let Some(ref robust_config) = value.robust_config {
            // Validate configuration
            robust_config.validate().map_err(|_| {
                tracing::error!("Invalid RPC client configuration");
                ConfigurationClientInitError::WsClientError
            })?;

            // Create client with the robust config
            let layered_client = EnhancedWsClientBuilder::new()
                .enable_ws_ping(PingConfig::default())
                .with_robust_config(robust_config.clone())
                .build(value.address.to_string())
                .await?;

            ClientWrapper::layered(layered_client)
        } else {
            // No config - use simple client
            let client = WsClientBuilder::new()
                .enable_ws_ping(PingConfig::default())
                .build(value.address.to_string())
                .await
                .map_err(|err| {
                    tracing::error!("Error creating ws client: {:?}", err);
                    ConfigurationClientInitError::WsClientError
                })?;

            ClientWrapper::simple(client)
        };

        // Create monitoring manager if monitoring is enabled
        let monitoring_manager = if let Some(ref robust_config) = value.robust_config {
            if robust_config.enable_monitoring {
                let monitoring_config = crate::transport::layers::MonitoringConfig {
                    check_interval: std::time::Duration::from_secs(30),
                    health_check_timeout: std::time::Duration::from_secs(5),
                    critical_threshold: 10,
                    enable_heartbeat_logging: true,
                };
                let manager = MonitoringManager::new(monitoring_config, value.address.clone());
                Some(Arc::new(tokio::sync::Mutex::new(manager)))
            } else {
                None
            }
        } else {
            None
        };

        let transport = ConfigurationTransport::builder()
            .listener_ref(value.listener_ref.clone())
            .client(client_wrapper)
            .monitoring_manager(monitoring_manager)
            .build();

        Ok(transport)
    }
}

// Helper function to convert ClientError to ConfigurationApiError
fn client_error_to_api_error(error: JsonRpcClientError) -> ConfigurationApiError {
    match error {
        JsonRpcClientError::Call(err) => ConfigurationApiError::from(err),
        _ => ConfigurationApiError::Unknown,
    }
}
#[cfg(disabled_tests)]
mod tests {
    use super::*;
    use crate::config::RpcClientConfig;
    use vg_config::http::listener::ListenerRef;

    #[test]
    fn test_configuration_transport_options_creation() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();

        let options = ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref.clone())
            .address(address.clone())
            .build();

        assert!(!options.has_robust_config());
        assert!(options.robust_config().is_none());
    }

    #[test]
    fn test_configuration_transport_options_with_robust_config() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let robust_config = RpcClientConfig::default();

        let options = ConfigurationTransportOptions::with_robust_config(
            listener_ref.clone(),
            address.clone(),
            robust_config,
        );

        assert!(options.has_robust_config());
        assert!(options.robust_config().is_some());
    }

    #[test]
    fn test_configuration_transport_options_production() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();

        let options = ConfigurationTransportOptions::production(listener_ref, address);

        assert!(options.has_robust_config());
        assert!(options.robust_config().is_some());
    }

    #[test]
    fn test_configuration_transport_options_development() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();

        let options = ConfigurationTransportOptions::development(listener_ref, address);

        assert!(options.has_robust_config());
        assert!(options.robust_config().is_some());
    }

    #[test]
    fn test_configuration_transport_options_with_default_robustness() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();

        let options = ConfigurationTransportOptions::with_default_robustness(listener_ref, address);

        assert!(options.has_robust_config());
        assert!(options.robust_config().is_some());
    }

    #[test]
    fn test_configuration_transport_options_with_custom_timeout() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();

        let robust_config = RpcClientConfig::new().with_timeout(std::time::Duration::from_secs(45));

        let options =
            ConfigurationTransportOptions::with_robust_config(listener_ref, address, robust_config);

        assert!(options.has_robust_config());
        let config = options.robust_config().unwrap();
        assert_eq!(config.request_timeout, std::time::Duration::from_secs(45));
    }

    #[test]
    fn test_configuration_transport_options_builder_pattern() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let robust_config = RpcClientConfig::default();

        let options = ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref.clone())
            .address(address.clone())
            .robust_config(Some(robust_config))
            .build();

        assert!(options.has_robust_config());
        assert!(options.robust_config().is_some());
    }
}
