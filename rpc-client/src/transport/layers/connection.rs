use crate::api::{ConfigurationClientError, ConnectionError};
use crate::config::ReconnectionConfig;
use crate::instrumentation::ClientMetrics;
use async_trait::async_trait;
use http::Uri;
use jsonrpsee::ws_client::{PingConfig, WsClient, WsClientBuilder};
use opentelemetry::KeyValue;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Connection manager handles WebSocket connection lifecycle and automatic reconnection
pub struct ConnectionManager {
    /// Current connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Factory for creating new clients
    client_factory: Arc<dyn ClientFactory>,
    /// Reconnection configuration
    config: ReconnectionConfig,
    /// Metrics for tracking connection events
    metrics: Option<Arc<ClientMetrics>>,
    /// Channel for sending reconnection commands
    reconnect_tx: Arc<Mutex<Option<mpsc::UnboundedSender<ReconnectCommand>>>>,
    /// Request queue for handling requests during reconnection
    request_queue: Arc<Mutex<Vec<QueuedRequest>>>,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(client_factory: Arc<dyn ClientFactory>, config: ReconnectionConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            client_factory,
            config,
            metrics: None,
            reconnect_tx: Arc::new(Mutex::new(None)),
            request_queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a new connection manager with metrics
    pub fn with_metrics(
        client_factory: Arc<dyn ClientFactory>,
        config: ReconnectionConfig,
        metrics: Arc<ClientMetrics>,
    ) -> Self {
        let mut manager = Self::new(client_factory, config);
        manager.metrics = Some(metrics);
        manager
    }

    /// Initialize the connection (if not lazy) or prepare for lazy connection
    pub async fn initialize(&self, uri: &Uri) -> Result<(), ConfigurationClientError> {
        if self.config.enable_lazy_connection {
            debug!("Lazy connection enabled, connection will be established on first request");
            return Ok(());
        }

        self.connect(uri).await
    }

    /// Establish connection to the server
    pub async fn connect(&self, uri: &Uri) -> Result<(), ConfigurationClientError> {
        {
            let mut state = self.state.write().unwrap();
            *state = ConnectionState::Connecting;
        }

        self.update_connection_metrics(ConnectionState::Connecting);

        match self.client_factory.create_client(uri).await {
            Ok(client) => {
                {
                    let mut state = self.state.write().unwrap();
                    *state = ConnectionState::Connected(Arc::new(client));
                }

                self.update_connection_metrics(ConnectionState::Connected(Arc::new(
                    // Placeholder for metrics - we can't clone the actual client
                    self.client_factory.create_client(uri).await.unwrap(),
                )));

                info!(uri = %uri, "Successfully connected to configuration server");

                // Process any queued requests
                self.process_queued_requests().await;

                Ok(())
            }
            Err(err) => {
                {
                    let mut state = self.state.write().unwrap();
                    *state = ConnectionState::Disconnected;
                }

                self.update_connection_metrics(ConnectionState::Disconnected);

                error!(uri = %uri, error = %err, "Failed to connect to configuration server");

                // Start reconnection process if configured
                if self.config.max_reconnect_attempts.is_some() {
                    self.start_reconnection_process(uri.clone()).await;
                }

                Err(ConfigurationClientError::ConnectionUnavailable)
            }
        }
    }

    /// Get the current client if connected
    pub async fn get_client(&self) -> Result<Arc<WsClient>, ConfigurationClientError> {
        let state = self.state.read().unwrap();
        match &*state {
            ConnectionState::Connected(client) => Ok(Arc::clone(client)),
            ConnectionState::Connecting => {
                drop(state);
                // Wait a bit and retry
                sleep(Duration::from_millis(100)).await;
                let state = self.state.read().unwrap();
                match &*state {
                    ConnectionState::Connected(client) => Ok(Arc::clone(client)),
                    _ => Err(ConfigurationClientError::ConnectionUnavailable),
                }
            }
            ConnectionState::Disconnected | ConnectionState::Reconnecting { .. } => {
                Err(ConfigurationClientError::ConnectionUnavailable)
            }
        }
    }

    /// Handle connection loss and start reconnection if configured
    pub async fn handle_connection_loss(&self, uri: Uri) {
        warn!("Connection lost, handling reconnection");

        {
            let mut state = self.state.write().unwrap();
            *state = ConnectionState::Reconnecting { attempts: 0 };
        }

        self.update_connection_metrics(ConnectionState::Reconnecting { attempts: 0 });

        if let Some(metrics) = &self.metrics {
            metrics.reconnection_attempts_total.add(1, &[]);
        }

        self.start_reconnection_process(uri).await;
    }

    /// Start the reconnection process
    async fn start_reconnection_process(&self, uri: Uri) {
        let (tx, mut rx) = mpsc::unbounded_channel();

        {
            let mut reconnect_tx = self.reconnect_tx.lock().await;
            *reconnect_tx = Some(tx.clone());
        }

        let state = Arc::clone(&self.state);
        let client_factory = Arc::clone(&self.client_factory);
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        let request_queue = Arc::clone(&self.request_queue);

        tokio::spawn(async move {
            let mut attempt = 0;

            while let Some(command) = rx.recv().await {
                match command {
                    ReconnectCommand::Attempt => {
                        if let Some(max_attempts) = config.max_reconnect_attempts {
                            if attempt >= max_attempts {
                                warn!(
                                    attempt = attempt,
                                    max_attempts = max_attempts,
                                    "Maximum reconnection attempts exceeded"
                                );

                                {
                                    let mut state_guard = state.write().unwrap();
                                    *state_guard = ConnectionState::Disconnected;
                                }

                                // Reject all queued requests
                                Self::reject_queued_requests(&request_queue).await;
                                break;
                            }
                        }

                        let delay = config.delay_for_reconnect_attempt(attempt);
                        debug!(
                            attempt = attempt,
                            delay = ?delay,
                            "Attempting reconnection"
                        );

                        sleep(delay).await;

                        match client_factory.create_client(&uri).await {
                            Ok(client) => {
                                {
                                    let mut state_guard = state.write().unwrap();
                                    *state_guard = ConnectionState::Connected(Arc::new(client));
                                }

                                info!(
                                    attempt = attempt,
                                    uri = %uri,
                                    "Successfully reconnected to configuration server"
                                );

                                if let Some(metrics) = &metrics {
                                    metrics.active_connections.record(1, &[]);
                                }

                                // Process queued requests
                                Self::process_queued_requests_static(&request_queue).await;
                                break;
                            }
                            Err(err) => {
                                attempt += 1;

                                {
                                    let mut state_guard = state.write().unwrap();
                                    *state_guard =
                                        ConnectionState::Reconnecting { attempts: attempt };
                                }

                                warn!(
                                    attempt = attempt,
                                    error = %err,
                                    "Reconnection attempt failed"
                                );

                                if let Some(metrics) = &metrics {
                                    metrics
                                        .reconnection_attempts_total
                                        .add(1, &[KeyValue::new("result", "failed")]);
                                }

                                // Schedule next attempt
                                if let Some(max_attempts) = config.max_reconnect_attempts {
                                    if attempt < max_attempts {
                                        let _ = tx.send(ReconnectCommand::Attempt);
                                    }
                                } else {
                                    let _ = tx.send(ReconnectCommand::Attempt);
                                }
                            }
                        }
                    }
                    ReconnectCommand::Stop => {
                        debug!("Stopping reconnection process");
                        break;
                    }
                }
            }
        });

        // Start the first reconnection attempt
        if let Some(tx) = self.reconnect_tx.lock().await.as_ref() {
            let _ = tx.send(ReconnectCommand::Attempt);
        }
    }

    /// Queue a request during reconnection
    pub async fn queue_request(
        &self,
        request: QueuedRequest,
    ) -> Result<(), ConfigurationClientError> {
        if !self.config.queue_requests_during_reconnection {
            return Err(ConfigurationClientError::ConnectionUnavailable);
        }

        let mut queue = self.request_queue.lock().await;

        if queue.len() >= self.config.max_queued_requests {
            warn!(
                queue_size = queue.len(),
                max_size = self.config.max_queued_requests,
                "Request queue is full, rejecting request"
            );
            return Err(ConfigurationClientError::ServiceUnavailable);
        }

        queue.push(request);
        debug!(
            queue_size = queue.len(),
            "Request queued during reconnection"
        );
        Ok(())
    }

    /// Process all queued requests
    async fn process_queued_requests(&self) {
        Self::process_queued_requests_static(&self.request_queue).await;
    }

    /// Static version of process_queued_requests for use in spawned tasks
    async fn process_queued_requests_static(request_queue: &Arc<Mutex<Vec<QueuedRequest>>>) {
        let mut queue = request_queue.lock().await;
        let requests = std::mem::take(&mut *queue);
        drop(queue);

        if !requests.is_empty() {
            info!(count = requests.len(), "Processing queued requests");
        }

        for request in requests {
            // Send success signal to indicate the request can be retried
            let _ = request.response_tx.send(Ok(()));
        }
    }

    /// Reject all queued requests
    async fn reject_queued_requests(request_queue: &Arc<Mutex<Vec<QueuedRequest>>>) {
        let mut queue = request_queue.lock().await;
        let requests = std::mem::take(&mut *queue);
        drop(queue);

        if !requests.is_empty() {
            warn!(
                count = requests.len(),
                "Rejecting queued requests due to connection failure"
            );
        }

        for request in requests {
            let _ = request
                .response_tx
                .send(Err(ConfigurationClientError::ConnectionUnavailable));
        }
    }

    /// Get current connection status
    pub fn get_connection_status(&self) -> ConnectionStatus {
        let state = self.state.read().unwrap();
        match &*state {
            ConnectionState::Disconnected => ConnectionStatus::Disconnected,
            ConnectionState::Connecting => ConnectionStatus::Connecting,
            ConnectionState::Connected(_) => ConnectionStatus::Connected,
            ConnectionState::Reconnecting { attempts } => ConnectionStatus::Reconnecting {
                attempts: *attempts,
            },
        }
    }

    /// Update connection metrics
    fn update_connection_metrics(&self, state: ConnectionState) {
        if let Some(metrics) = &self.metrics {
            let connection_count = match state {
                ConnectionState::Connected(_) => 1,
                _ => 0,
            };
            metrics.active_connections.record(connection_count, &[]);
        }
    }

    /// Stop the connection manager and cleanup resources
    pub async fn stop(&self) {
        if let Some(tx) = self.reconnect_tx.lock().await.as_ref() {
            let _ = tx.send(ReconnectCommand::Stop);
        }

        {
            let mut state = self.state.write().unwrap();
            *state = ConnectionState::Disconnected;
        }

        self.update_connection_metrics(ConnectionState::Disconnected);

        // Reject any remaining queued requests
        Self::reject_queued_requests(&self.request_queue).await;
    }
}

/// Factory trait for creating WebSocket clients
#[async_trait]
pub trait ClientFactory: Send + Sync {
    async fn create_client(&self, uri: &Uri) -> Result<WsClient, ClientError>;
}

/// Default implementation of ClientFactory
pub struct DefaultClientFactory;

#[async_trait]
impl ClientFactory for DefaultClientFactory {
    async fn create_client(&self, uri: &Uri) -> Result<WsClient, ClientError> {
        WsClientBuilder::new()
            .enable_ws_ping(PingConfig::default())
            .build(uri.to_string())
            .await
            .map_err(|err| ClientError::Connection(err.to_string()))
    }
}

/// Connection state enumeration
#[derive(Debug, Clone)]
pub enum ConnectionState {
    /// Not connected to the server
    Disconnected,
    /// Currently establishing connection
    Connecting,
    /// Successfully connected with active client
    Connected(Arc<WsClient>),
    /// Attempting to reconnect after connection loss
    Reconnecting { attempts: u32 },
}

/// Public connection status for monitoring
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConnectionStatus {
    /// Not connected to the server
    Disconnected,
    /// Currently establishing connection
    Connecting,
    /// Successfully connected
    Connected,
    /// Attempting to reconnect after connection loss
    Reconnecting { attempts: u32 },
}

/// Commands for controlling reconnection process
#[derive(Debug)]
enum ReconnectCommand {
    /// Attempt a reconnection
    Attempt,
    /// Stop the reconnection process
    Stop,
}

/// A queued request during reconnection
pub struct QueuedRequest {
    /// Timestamp when the request was queued
    pub queued_at: Instant,
    /// Channel to send the response back
    pub response_tx: oneshot::Sender<Result<(), ConfigurationClientError>>,
}

impl QueuedRequest {
    /// Create a new queued request
    pub fn new(response_tx: oneshot::Sender<Result<(), ConfigurationClientError>>) -> Self {
        Self {
            queued_at: Instant::now(),
            response_tx,
        }
    }
}

/// Client error types for the factory
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl From<ClientError> for ConfigurationClientError {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::Connection(_) => ConfigurationClientError::ConnectionUnavailable,
            ClientError::Configuration(msg) => ConfigurationClientError::Unknown(
                crate::api::SourceError::from(Box::new(ConnectionError { message: msg })
                    as Box<dyn std::error::Error + Send + Sync>),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::timeout;

    /// Mock client factory for testing
    struct MockClientFactory {
        should_fail: Arc<AtomicBool>,
        fail_count: Arc<std::sync::atomic::AtomicU32>,
    }

    impl MockClientFactory {
        fn new() -> Self {
            Self {
                should_fail: Arc::new(AtomicBool::new(false)),
                fail_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            }
        }

        fn set_should_fail(&self, should_fail: bool) {
            self.should_fail.store(should_fail, Ordering::Relaxed);
        }

        fn get_fail_count(&self) -> u32 {
            self.fail_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl ClientFactory for MockClientFactory {
        async fn create_client(&self, _uri: &Uri) -> Result<WsClient, ClientError> {
            if self.should_fail.load(Ordering::Relaxed) {
                self.fail_count.fetch_add(1, Ordering::Relaxed);
                return Err(ClientError::Connection(
                    "Mock connection failure".to_string(),
                ));
            }

            // For testing, we can't create a real WsClient, so we'll simulate success
            // In a real test environment, you'd use a mock server
            Err(ClientError::Connection(
                "Mock - cannot create real client in test".to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let factory = Arc::new(MockClientFactory::new());
        let config = ReconnectionConfig::default();
        let manager = ConnectionManager::new(factory, config);

        assert_eq!(
            manager.get_connection_status(),
            ConnectionStatus::Disconnected
        );
    }

    #[tokio::test]
    async fn test_connection_status_transitions() {
        let factory = Arc::new(MockClientFactory::new());
        let config = ReconnectionConfig::builder()
            .enable_lazy_connection(true)
            .build();
        let manager = ConnectionManager::new(factory, config);

        // Initially disconnected
        assert_eq!(
            manager.get_connection_status(),
            ConnectionStatus::Disconnected
        );

        // Test lazy connection initialization
        let uri: Uri = "ws://localhost:8080".parse().unwrap();
        let result = manager.initialize(&uri).await;
        assert!(result.is_ok());
        assert_eq!(
            manager.get_connection_status(),
            ConnectionStatus::Disconnected
        );
    }

    #[tokio::test]
    async fn test_request_queueing() {
        let factory = Arc::new(MockClientFactory::new());
        let config = ReconnectionConfig::builder()
            .queue_requests_during_reconnection(true)
            .max_queued_requests(2)
            .build();
        let manager = ConnectionManager::new(factory, config);

        // Queue a request
        let (tx, _rx) = oneshot::channel();
        let request = QueuedRequest::new(tx);
        let result = manager.queue_request(request).await;
        assert!(result.is_ok());

        // Queue another request
        let (tx2, _rx2) = oneshot::channel();
        let request2 = QueuedRequest::new(tx2);
        let result2 = manager.queue_request(request2).await;
        assert!(result2.is_ok());

        // Third request should fail (queue full)
        let (tx3, _rx3) = oneshot::channel();
        let request3 = QueuedRequest::new(tx3);
        let result3 = manager.queue_request(request3).await;
        assert!(matches!(
            result3,
            Err(ConfigurationClientError::ServiceUnavailable)
        ));
    }

    #[tokio::test]
    async fn test_queue_disabled() {
        let factory = Arc::new(MockClientFactory::new());
        let config = ReconnectionConfig::builder()
            .queue_requests_during_reconnection(false)
            .build();
        let manager = ConnectionManager::new(factory, config);

        // Queueing should be disabled
        let (tx, _rx) = oneshot::channel();
        let request = QueuedRequest::new(tx);
        let result = manager.queue_request(request).await;
        assert!(matches!(
            result,
            Err(ConfigurationClientError::ConnectionUnavailable)
        ));
    }

    #[tokio::test]
    async fn test_connection_manager_stop() {
        let factory = Arc::new(MockClientFactory::new());
        let config = ReconnectionConfig::default();
        let manager = ConnectionManager::new(factory, config);

        // Stop should complete without hanging
        let stop_result = timeout(Duration::from_secs(1), manager.stop()).await;
        assert!(stop_result.is_ok());
        assert_eq!(
            manager.get_connection_status(),
            ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_queued_request_creation() {
        let (tx, _rx) = oneshot::channel();
        let request = QueuedRequest::new(tx);

        // Should have a recent timestamp
        let elapsed = request.queued_at.elapsed();
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    fn test_connection_status_equality() {
        assert_eq!(ConnectionStatus::Connected, ConnectionStatus::Connected);
        assert_eq!(
            ConnectionStatus::Disconnected,
            ConnectionStatus::Disconnected
        );
        assert_eq!(
            ConnectionStatus::Reconnecting { attempts: 1 },
            ConnectionStatus::Reconnecting { attempts: 1 }
        );
        assert_ne!(
            ConnectionStatus::Reconnecting { attempts: 1 },
            ConnectionStatus::Reconnecting { attempts: 2 }
        );
    }

    #[test]
    fn test_client_error_conversions() {
        let conn_error = ClientError::Connection("test".to_string());
        let client_error: ConfigurationClientError = conn_error.into();
        assert!(matches!(
            client_error,
            ConfigurationClientError::ConnectionUnavailable
        ));

        let config_error = ClientError::Configuration("test config".to_string());
        let client_error2: ConfigurationClientError = config_error.into();
        assert!(matches!(
            client_error2,
            ConfigurationClientError::Unknown(_)
        ));
    }
}
