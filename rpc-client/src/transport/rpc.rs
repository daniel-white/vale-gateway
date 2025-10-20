use getset::Getters;
use http::Uri;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_core::sync::{broadcast, handles::Handle};

use crate::config::RobustClientConfig;
use crate::transport::layers::{LayeredClient, MonitoringManager};

/// Error types for RPC transport operations
#[derive(Debug, Error)]
pub enum RpcTransportError {
    /// Transport initialization failed
    #[error("Transport initialization failed: {0}")]
    InitializationFailed(#[from] crate::transport::ConfigurationClientInitError),

    /// RPC transport already initialized with different configuration
    #[error("RPC transport already initialized with different configuration")]
    ConfigurationMismatch,

    /// Monitoring error
    #[error("Monitoring error: {0}")]
    MonitoringError(#[from] crate::transport::layers::MonitoringError),
}

/// Configuration for RPC transport
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct RpcTransportConfig {
    /// Robust client configuration
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

impl From<crate::transport::ConfigurationTransportOptions> for RpcTransportConfig {
    fn from(options: crate::transport::ConfigurationTransportOptions) -> Self {
        Self::builder()
            .robust_config(options.robust_config().cloned().unwrap_or_default())
            .address(options.address().clone())
            .event_buffer_size(1024)
            .build()
    }
}

/// RPC transport that provides shared connection for both configuration and events clients
#[derive(Clone, Getters, TypedBuilder)]
pub struct RpcTransport {
    /// Reference to shared LayeredClient
    #[getset(get = "pub")]
    client: std::sync::Arc<LayeredClient>,

    /// Connection configuration
    #[getset(get = "pub")]
    config: RpcTransportConfig,

    /// Connection handle using core utilities
    #[getset(get = "pub")]
    connection_handle: Handle,

    /// Event distribution using core broadcast channels
    #[getset(get = "pub")]
    event_sender: broadcast::Sender<vg_rpc::ConfigurationEvent>,

    /// Monitoring manager
    #[getset(get = "pub")]
    monitoring_manager: std::sync::Arc<tokio::sync::Mutex<MonitoringManager>>,
}

impl RpcTransport {
    /// Create a new RPC transport
    pub async fn new(
        _address: Uri,
        _robust_config: RobustClientConfig,
    ) -> Result<Self, RpcTransportError> {
        // This will be implemented in the next task
        todo!("RpcTransport::new implementation will be added in task 2.1")
    }
}
