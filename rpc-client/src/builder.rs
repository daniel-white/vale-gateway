use crate::transport::layers::{LayeredClient, WsClientLayer};
use crate::{ConfigurationClientInitError, RpcClientConfig};
use jsonrpsee::ws_client::{PingConfig, WsClient, WsClientBuilder};

/// Enhanced WebSocket client builder that supports Tower middleware layers
/// for implementing robustness features like timeouts, retries, and circuit breakers.
pub struct EnhancedWsClientBuilder {
    /// The underlying jsonrpsee WsClientBuilder
    builder: WsClientBuilder,
    /// Stack of middleware layers to apply to the client
    layers: Vec<Box<dyn WsClientLayer>>,
    /// Optional RPC client configuration
    robust_config: Option<RpcClientConfig>,
}

impl EnhancedWsClientBuilder {
    /// Create a new enhanced builder with default settings
    pub fn new() -> Self {
        Self {
            builder: WsClientBuilder::new(),
            layers: Vec::new(),
            robust_config: None,
        }
    }

    /// Enable WebSocket ping with default configuration
    pub fn enable_ws_ping(mut self, config: PingConfig) -> Self {
        self.builder = self.builder.enable_ws_ping(config);
        self
    }

    /// Set the maximum request size
    pub fn max_request_size(mut self, size: u32) -> Self {
        self.builder = self.builder.max_request_size(size);
        self
    }

    /// Set the maximum response size
    pub fn max_response_size(mut self, size: u32) -> Self {
        self.builder = self.builder.max_response_size(size);
        self
    }

    /// Set the connection timeout
    pub fn connection_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.builder = self.builder.connection_timeout(timeout);
        self
    }

    /// Set the request timeout
    pub fn request_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.builder = self.builder.request_timeout(timeout);
        self
    }

    /// Add a middleware layer to the client stack
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: WsClientLayer + 'static,
    {
        self.layers.push(Box::new(layer));
        self
    }

    /// Configure RPC client features (timeouts, monitoring, metrics)
    pub fn with_robust_config(mut self, config: RpcClientConfig) -> Self {
        self.robust_config = Some(config);
        self
    }

    /// Apply robustness configuration by adding appropriate layers
    fn apply_robust_config(&mut self) {
        if let Some(config) = &self.robust_config {
            // For the simplified config, we just set the request timeout on the builder
            let builder = std::mem::take(&mut self.builder);
            self.builder = builder.request_timeout(config.request_timeout);

            tracing::debug!(
                "RPC client configured with timeout: {:?}, monitoring: {}, metrics: {}",
                config.request_timeout,
                config.enable_monitoring,
                config.enable_metrics
            );
        }
    }

    /// Build the enhanced client with all configured layers
    pub async fn build(
        mut self,
        uri: impl AsRef<str>,
    ) -> Result<LayeredClient, ConfigurationClientInitError> {
        // Apply robustness configuration
        self.apply_robust_config();

        // Build the base WsClient
        let base_client = self.builder.build(uri.as_ref()).await.map_err(|err| {
            tracing::error!("Failed to create WebSocket client: {:?}", err);
            ConfigurationClientInitError::WsClientError
        })?;

        // Performance optimization: Use optimized layer application
        let layered_client = LayeredClient::new_optimized(base_client, self.layers);

        Ok(layered_client)
    }

    /// Build a simple client without layers (for backward compatibility)
    pub async fn build_simple(
        self,
        uri: impl AsRef<str>,
    ) -> Result<WsClient, ConfigurationClientInitError> {
        self.builder.build(uri.as_ref()).await.map_err(|err| {
            tracing::error!("Failed to create WebSocket client: {:?}", err);
            ConfigurationClientInitError::WsClientError
        })
    }

    /// Get the number of configured layers
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Check if robustness features are configured
    pub fn has_robust_config(&self) -> bool {
        self.robust_config.is_some()
    }
}

impl Default for EnhancedWsClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder methods for creating common configurations
impl EnhancedWsClientBuilder {
    /// Create a builder with default configuration enabled
    pub fn with_default_config() -> Self {
        Self::new().with_robust_config(RpcClientConfig::default())
    }

    /// Create a builder optimized for production use
    pub fn production() -> Self {
        Self::new()
            .enable_ws_ping(PingConfig::default())
            .connection_timeout(std::time::Duration::from_secs(10))
            .request_timeout(std::time::Duration::from_secs(30))
            .with_robust_config(RpcClientConfig::default())
    }

    /// Create a builder optimized for development/testing
    pub fn development() -> Self {
        Self::new()
            .enable_ws_ping(PingConfig::default())
            .connection_timeout(std::time::Duration::from_secs(5))
            .request_timeout(std::time::Duration::from_secs(10))
            .with_robust_config(
                RpcClientConfig::new().with_timeout(std::time::Duration::from_secs(10)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_builder_creation() {
        let builder = EnhancedWsClientBuilder::new();
        assert_eq!(builder.layer_count(), 0);
        assert!(!builder.has_robust_config());
    }

    #[test]
    fn test_builder_with_robust_config() {
        let config = RpcClientConfig::default();
        let builder = EnhancedWsClientBuilder::new().with_robust_config(config);
        assert!(builder.has_robust_config());
    }

    #[test]
    fn test_production_builder() {
        let builder = EnhancedWsClientBuilder::production();
        assert!(builder.has_robust_config());
    }

    #[test]
    fn test_development_builder() {
        let builder = EnhancedWsClientBuilder::development();
        assert!(builder.has_robust_config());
    }

    #[test]
    fn test_default_robustness_builder() {
        let builder = EnhancedWsClientBuilder::with_default_config();
        assert!(builder.has_robust_config());
    }

    #[test]
    fn test_builder_with_timeout_config() {
        use crate::config::RpcClientConfig;

        let robust_config = RpcClientConfig::new().with_timeout(std::time::Duration::from_secs(45));

        let builder = EnhancedWsClientBuilder::new().with_robust_config(robust_config);

        assert!(builder.has_robust_config());
    }

    #[test]
    fn test_builder_layer_addition() {
        use crate::layers::NoOpLayer;

        let builder = EnhancedWsClientBuilder::new().layer(NoOpLayer);

        assert_eq!(builder.layer_count(), 1);
    }

    #[test]
    fn test_builder_multiple_layers() {
        use crate::layers::{NoOpLayer, TimeoutLayer};

        let builder = EnhancedWsClientBuilder::new()
            .layer(NoOpLayer)
            .layer(TimeoutLayer::new(std::time::Duration::from_secs(30)));

        assert_eq!(builder.layer_count(), 2);
    }
}
