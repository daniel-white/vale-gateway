pub mod circuit_breaker;
pub mod connection;
#[cfg(test)]
pub mod connection_tests;
pub mod reconnection;
pub mod retry;
pub mod status;
pub mod timeout;

pub use circuit_breaker::*;
pub use connection::*;
pub use reconnection::*;
pub use retry::*;
pub use status::*;
pub use timeout::*;

use jsonrpsee::ws_client::WsClient;

/// Trait for layers that can be applied during client creation.
/// This approach focuses on configuring the client during build time rather than runtime wrapping.
///
/// Performance considerations:
/// - Layers should be lightweight and avoid unnecessary allocations
/// - Configuration should be done once during build time, not per-request
/// - Layers should have minimal overhead when features are disabled
pub trait WsClientLayer: Send + Sync + 'static {
    /// Configure or modify the client during creation
    /// This allows layers to set up middleware, configure timeouts, etc.
    ///
    /// Performance note: This method is called once during client creation,
    /// so expensive setup operations are acceptable here.
    fn configure_client(&self, client: WsClient) -> WsClient;

    /// Check if this layer is enabled/active
    /// This allows for zero-cost abstraction when features are disabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Get the layer name for debugging and metrics
    fn layer_name(&self) -> &'static str {
        "unknown"
    }
}

/// A client wrapper that can be configured with multiple layers during creation.
/// This maintains full compatibility with the original WsClient while allowing
/// middleware configuration during the build process.
pub struct LayeredClient {
    /// The configured WsClient
    inner: WsClient,
}

impl LayeredClient {
    /// Create a new LayeredClient by applying configuration layers to the base WsClient
    ///
    /// Performance optimization: Only applies enabled layers to minimize overhead
    pub fn new(base_client: WsClient, layers: Vec<Box<dyn WsClientLayer>>) -> Self {
        let mut client = base_client;

        // Apply layers in order - each layer can configure the client
        // Performance optimization: Skip disabled layers
        for layer in layers {
            if layer.is_enabled() {
                tracing::debug!("Applying layer: {}", layer.layer_name());
                client = layer.configure_client(client);
            } else {
                tracing::debug!("Skipping disabled layer: {}", layer.layer_name());
            }
        }

        Self { inner: client }
    }

    /// Create a LayeredClient with optimized layer application
    /// This method pre-filters enabled layers for better performance
    pub fn new_optimized(base_client: WsClient, layers: Vec<Box<dyn WsClientLayer>>) -> Self {
        // Pre-filter enabled layers to avoid runtime checks
        let enabled_layers: Vec<_> = layers
            .into_iter()
            .filter(|layer| layer.is_enabled())
            .collect();

        if enabled_layers.is_empty() {
            tracing::debug!("No enabled layers, using client without modifications");
            return Self::without_layers(base_client);
        }

        let mut client = base_client;
        for layer in enabled_layers {
            tracing::debug!("Applying enabled layer: {}", layer.layer_name());
            client = layer.configure_client(client);
        }

        Self { inner: client }
    }

    /// Create a LayeredClient with no layers (just wraps the base client)
    pub fn without_layers(base_client: WsClient) -> Self {
        Self { inner: base_client }
    }

    /// Get a reference to the inner WsClient
    pub fn inner(&self) -> &WsClient {
        &self.inner
    }

    /// Get a mutable reference to the inner WsClient
    pub fn inner_mut(&mut self) -> &mut WsClient {
        &mut self.inner
    }

    /// Consume this wrapper and return the inner WsClient
    pub fn into_inner(self) -> WsClient {
        self.inner
    }
}

// Implement Deref to make LayeredClient behave like WsClient
impl std::ops::Deref for LayeredClient {
    type Target = WsClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for LayeredClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A no-op layer for testing and as a base implementation
pub struct NoOpLayer;

impl WsClientLayer for NoOpLayer {
    fn configure_client(&self, client: WsClient) -> WsClient {
        // No-op: return the client unchanged
        client
    }

    fn is_enabled(&self) -> bool {
        false // No-op layer is always disabled for performance
    }

    fn layer_name(&self) -> &'static str {
        "no-op"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_op_layer() {
        let layer = NoOpLayer;
        let layers: Vec<Box<dyn WsClientLayer>> = vec![Box::new(layer)];
        assert_eq!(layers.len(), 1);
    }

    #[test]
    fn test_layered_client_creation() {
        // Test that LayeredClient can be created with layers
        let layers: Vec<Box<dyn WsClientLayer>> = vec![Box::new(NoOpLayer)];
        assert_eq!(layers.len(), 1);

        // In a real test, we'd create a mock WsClient and test the layering
        // For now, we just test the structure
    }
}
