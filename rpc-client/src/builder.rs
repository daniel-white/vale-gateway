use crate::transport::layers::{LayeredClient, WsClientLayer};
use crate::{ConfigurationClientInitError, RobustClientConfig};
use jsonrpsee::ws_client::{PingConfig, WsClient, WsClientBuilder};

/// Enhanced WebSocket client builder that supports Tower middleware layers
/// for implementing robustness features like timeouts, retries, and circuit breakers.
pub struct EnhancedWsClientBuilder {
    /// The underlying jsonrpsee WsClientBuilder
    builder: WsClientBuilder,
    /// Stack of middleware layers to apply to the client
    layers: Vec<Box<dyn WsClientLayer>>,
    /// Optional robustness configuration
    robust_config: Option<RobustClientConfig>,
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

    /// Configure robustness features (timeouts, retries, circuit breaker, etc.)
    pub fn with_robust_config(mut self, config: RobustClientConfig) -> Self {
        self.robust_config = Some(config);
        self
    }

    /// Apply robustness configuration by adding appropriate layers
    fn apply_robust_config(&mut self) {
        if let Some(config) = &self.robust_config {
            // Add layers based on configuration in order of application

            // Add timeout layer if configured
            if let Some(timeout_config) = &config.timeout {
                use crate::layers::TimeoutLayer;

                let timeout_layer = TimeoutLayer::from_config(timeout_config);
                self.layers.push(Box::new(timeout_layer));

                tracing::debug!(
                    "Timeout layer added with default timeout: {:?}",
                    timeout_config.default_timeout
                );
            }

            // Add circuit breaker layer if configured
            if let Some(circuit_breaker_config) = &config.circuit_breaker {
                use crate::layers::CircuitBreakerLayer;

                let circuit_breaker_layer =
                    CircuitBreakerLayer::new(circuit_breaker_config.clone());
                self.layers.push(Box::new(circuit_breaker_layer));

                tracing::debug!(
                    "Circuit breaker layer added with failure threshold: {}",
                    circuit_breaker_config.failure_threshold
                );
            }

            // Add retry layer if configured
            if let Some(retry_config) = &config.retry {
                use crate::layers::RetryLayer;

                let retry_layer = RetryLayer::from_config(retry_config);
                self.layers.push(Box::new(retry_layer));

                tracing::debug!(
                    "Retry layer added with max attempts: {}",
                    retry_config.max_attempts
                );
            }

            // Add reconnection layer if configured
            if let Some(reconnection_config) = &config.reconnection {
                use crate::layers::ReconnectionLayer;

                let reconnection_layer = ReconnectionLayer::new(reconnection_config.clone());
                self.layers.push(Box::new(reconnection_layer));

                tracing::debug!(
                    "Reconnection layer added with max attempts: {:?}",
                    reconnection_config.max_reconnect_attempts
                );
            }

            // Add instrumentation layer if configured (should be last to capture all metrics)
            if config.instrumentation.enable_metrics || config.instrumentation.enable_tracing {
                // Note: Instrumentation is handled through tracing and metrics in other layers
                // rather than a dedicated layer since ConnectionLogger doesn't implement WsClientLayer
                tracing::debug!(
                    "Instrumentation enabled - metrics and tracing will be handled by other layers"
                );
            }
        }
    }

    /// Build the enhanced client with all configured layers
    pub async fn build(
        mut self,
        uri: impl AsRef<str>,
    ) -> Result<LayeredClient, ConfigurationClientInitError> {
        // Apply timeout configuration to the builder if configured
        if let Some(config) = &self.robust_config
            && let Some(timeout_config) = &config.timeout
        {
            self.builder = self.builder.request_timeout(timeout_config.default_timeout);
        }

        // Apply robustness configuration (add layers)
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
    /// Create a builder with default robustness features enabled
    pub fn with_default_robustness() -> Self {
        Self::new().with_robust_config(RobustClientConfig::default())
    }

    /// Create a builder optimized for production use
    pub fn production() -> Self {
        Self::new()
            .enable_ws_ping(PingConfig::default())
            .connection_timeout(std::time::Duration::from_secs(10))
            .request_timeout(std::time::Duration::from_secs(30))
            .with_robust_config(RobustClientConfig::production())
    }

    /// Create a builder optimized for development/testing
    pub fn development() -> Self {
        Self::new()
            .enable_ws_ping(PingConfig::default())
            .connection_timeout(std::time::Duration::from_secs(5))
            .request_timeout(std::time::Duration::from_secs(10))
            .with_robust_config(RobustClientConfig::development())
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
        let config = RobustClientConfig::default();
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
        let builder = EnhancedWsClientBuilder::with_default_robustness();
        assert!(builder.has_robust_config());
    }

    #[test]
    fn test_builder_with_timeout_config() {
        use crate::config::{RobustClientConfig, TimeoutConfig};

        let timeout_config = TimeoutConfig::builder()
            .default_timeout(std::time::Duration::from_secs(45))
            .build();

        let robust_config = RobustClientConfig::builder()
            .timeout(Some(timeout_config))
            .build();

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
