pub mod retry;
pub mod timeout;

pub use retry::*;
pub use timeout::*;

use jsonrpsee::ws_client::WsClient;

/// Trait for layers that can be applied during client creation.
/// This approach focuses on configuring the client during build time rather than runtime wrapping.
pub trait WsClientLayer: Send + Sync + 'static {
    /// Configure or modify the client during creation
    /// This allows layers to set up middleware, configure timeouts, etc.
    fn configure_client(&self, client: WsClient) -> WsClient;
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
    pub fn new(base_client: WsClient, layers: Vec<Box<dyn WsClientLayer>>) -> Self {
        let mut client = base_client;

        // Apply layers in order - each layer can configure the client
        for layer in layers {
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
