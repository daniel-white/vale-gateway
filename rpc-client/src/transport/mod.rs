use async_from::AsyncTryFrom;
use async_trait::async_trait;

use http::Uri;
use jsonrpsee::core::ClientError;
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

use crate::transport::layers::LayeredClient;
use crate::{EnhancedWsClientBuilder, RobustClientConfig};

pub mod layers;

pub use layers::*;

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
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationTransportOptions {
    #[builder(setter(into))]
    listener_ref: ListenerRef,
    #[builder(setter(into))]
    address: Uri,
    /// Optional robustness configuration for enhanced client features
    #[builder(default)]
    robust_config: Option<RobustClientConfig>,
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
        robust_config: RobustClientConfig,
    ) -> Self {
        Self::builder()
            .listener_ref(listener_ref)
            .address(address)
            .robust_config(Some(robust_config))
            .build()
    }

    /// Create transport options with production-ready robustness settings
    pub fn production(listener_ref: impl Into<ListenerRef>, address: impl Into<Uri>) -> Self {
        Self::with_robust_config(listener_ref, address, RobustClientConfig::production())
    }

    /// Create transport options with development-friendly robustness settings
    pub fn development(listener_ref: impl Into<ListenerRef>, address: impl Into<Uri>) -> Self {
        Self::with_robust_config(listener_ref, address, RobustClientConfig::development())
    }

    /// Create transport options with default robustness settings
    pub fn with_default_robustness(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Self {
        Self::with_robust_config(listener_ref, address, RobustClientConfig::default())
    }

    /// Check if robustness features are configured
    pub fn has_robust_config(&self) -> bool {
        self.robust_config.is_some()
    }

    /// Get a reference to the robust configuration if present
    pub fn robust_config(&self) -> Option<&RobustClientConfig> {
        self.robust_config.as_ref()
    }
}

#[async_trait]
impl AsyncTryFrom<ConfigurationTransportOptions> for ConfigurationTransport {
    type Error = ConfigurationClientInitError;

    async fn async_try_from(value: ConfigurationTransportOptions) -> Result<Self, Self::Error> {
        let client_wrapper = if let Some(robust_config) = value.robust_config {
            // Validate configuration before proceeding
            robust_config.validate().map_err(|_| {
                tracing::error!("Invalid robust client configuration");
                ConfigurationClientInitError::WsClientError
            })?;

            // Use EnhancedWsClientBuilder with robustness features
            let layered_client = EnhancedWsClientBuilder::new()
                .enable_ws_ping(PingConfig::default())
                .with_robust_config(robust_config)
                .build(value.address.to_string())
                .await?;

            ClientWrapper::layered(layered_client)
        } else {
            // Use standard WsClientBuilder for backward compatibility
            let simple_client = WsClientBuilder::new()
                .enable_ws_ping(PingConfig::default())
                .build(value.address.to_string())
                .await
                .map_err(|err| {
                    tracing::error!("Error creating ws client: {:?}", err);
                    ConfigurationClientInitError::WsClientError
                })?;

            ClientWrapper::simple(simple_client)
        };

        let transport = ConfigurationTransport::builder()
            .listener_ref(value.listener_ref)
            .client(client_wrapper)
            .build();

        Ok(transport)
    }
}

// Helper function to convert ClientError to ConfigurationApiError
fn client_error_to_api_error(error: ClientError) -> ConfigurationApiError {
    match error {
        ClientError::Call(err) => ConfigurationApiError::from(err),
        _ => ConfigurationApiError::Unknown,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RobustClientConfig, TimeoutConfig};
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
        let robust_config = RobustClientConfig::default();

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

        let timeout_config = TimeoutConfig::builder()
            .default_timeout(std::time::Duration::from_secs(45))
            .build();

        let robust_config = RobustClientConfig::builder()
            .timeout(Some(timeout_config))
            .build();

        let options =
            ConfigurationTransportOptions::with_robust_config(listener_ref, address, robust_config);

        assert!(options.has_robust_config());
        let config = options.robust_config().unwrap();
        assert!(config.timeout.is_some());
        assert_eq!(
            config.timeout.as_ref().unwrap().default_timeout,
            std::time::Duration::from_secs(45)
        );
    }

    #[test]
    fn test_configuration_transport_options_builder_pattern() {
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let address: Uri = "ws://localhost:8080".parse().unwrap();
        let robust_config = RobustClientConfig::default();

        let options = ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref.clone())
            .address(address.clone())
            .robust_config(Some(robust_config))
            .build();

        assert!(options.has_robust_config());
        assert!(options.robust_config().is_some());
    }
}
