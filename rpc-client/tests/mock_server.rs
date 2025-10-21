use async_trait::async_trait;
use jsonrpsee::PendingSubscriptionSink;
use jsonrpsee::core::SubscriptionResult;
use jsonrpsee::server::{ServerBuilder, ServerHandle};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ConfigurationApiError, ConfigurationApiServer, GetBackendRequest, GetListenerRequest,
    GetRouteRequest, GetSharedFilterRequest, SubscribeEventsRequest,
};

/// Configurable behavior for the mock server
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ServerBehavior {
    /// Normal operation - responds successfully
    Normal,
    /// Slow responses with configurable delay
    Slow { delay: Duration },
    /// Failing responses with configurable error rate (0.0 to 1.0)
    Failing { error_rate: f64 },
    /// Server is completely unavailable (doesn't respond)
    Unavailable,
    /// Custom behavior for specific methods
    Custom {
        listener_behavior: Option<MethodBehavior>,
        route_behavior: Option<MethodBehavior>,
        backend_behavior: Option<MethodBehavior>,
        shared_filter_behavior: Option<MethodBehavior>,
    },
}

/// Behavior for individual methods
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MethodBehavior {
    Normal,
    Slow { delay: Duration },
    Error { error: ConfigurationApiError },
    Timeout, // Never responds
}

impl Default for ServerBehavior {
    fn default() -> Self {
        Self::Normal
    }
}

/// Mock configuration server for testing
pub struct MockConfigurationServer {
    behavior: Arc<RwLock<ServerBehavior>>,
    data: Arc<RwLock<MockData>>,
    request_count: Arc<Mutex<HashMap<String, u32>>>,
    server_handle: Option<ServerHandle>,
}

/// Mock data storage for the server
#[derive(Debug, Default)]
struct MockData {
    listeners: HashMap<ListenerRef, Listener>,
    routes: HashMap<RouteRef, Route>,
    backends: HashMap<BackendRef, Backend>,
    shared_filters: HashMap<SharedFilterRef, SharedFilter>,
}

impl MockConfigurationServer {
    /// Create a new mock server with default behavior
    pub fn new() -> Self {
        Self {
            behavior: Arc::new(RwLock::new(ServerBehavior::Normal)),
            data: Arc::new(RwLock::new(MockData::default())),
            request_count: Arc::new(Mutex::new(HashMap::new())),
            server_handle: None,
        }
    }

    /// Create a mock server with specific behavior
    pub fn with_behavior(behavior: ServerBehavior) -> Self {
        Self {
            behavior: Arc::new(RwLock::new(behavior)),
            data: Arc::new(RwLock::new(MockData::default())),
            request_count: Arc::new(Mutex::new(HashMap::new())),
            server_handle: None,
        }
    }

    /// Start the mock server on a random available port
    pub async fn start(&mut self) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
        let server = ServerBuilder::default().build("127.0.0.1:0").await?;

        let addr = server.local_addr()?;

        let mock_service = MockConfigurationService {
            behavior: self.behavior.clone(),
            data: self.data.clone(),
            request_count: self.request_count.clone(),
        };

        let handle = server.start(mock_service.into_rpc());
        self.server_handle = Some(handle);

        Ok(addr)
    }

    /// Stop the mock server
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(handle) = self.server_handle.take() {
            handle.stop()?;
        }
        Ok(())
    }

    /// Change the server behavior at runtime
    pub async fn set_behavior(&self, behavior: ServerBehavior) {
        let mut current_behavior = self.behavior.write().await;
        *current_behavior = behavior;
    }

    /// Add mock data to the server
    pub async fn add_listener(&self, listener_ref: ListenerRef, listener: Listener) {
        let mut data = self.data.write().await;
        data.listeners.insert(listener_ref, listener);
    }

    #[allow(dead_code)]
    pub async fn add_route(&self, route_ref: RouteRef, route: Route) {
        let mut data = self.data.write().await;
        data.routes.insert(route_ref, route);
    }

    #[allow(dead_code)]
    pub async fn add_backend(&self, backend_ref: BackendRef, backend: Backend) {
        let mut data = self.data.write().await;
        data.backends.insert(backend_ref, backend);
    }

    #[allow(dead_code)]
    pub async fn add_shared_filter(&self, filter_ref: SharedFilterRef, filter: SharedFilter) {
        let mut data = self.data.write().await;
        data.shared_filters.insert(filter_ref, filter);
    }

    /// Get request count for a specific method
    pub async fn get_request_count(&self, method: &str) -> u32 {
        let counts = self.request_count.lock().await;
        counts.get(method).copied().unwrap_or(0)
    }

    /// Reset request counts
    pub async fn reset_request_counts(&self) {
        let mut counts = self.request_count.lock().await;
        counts.clear();
    }

    /// Simulate network partition (server becomes unavailable)
    #[allow(dead_code)]
    pub async fn simulate_network_partition(&self) {
        self.set_behavior(ServerBehavior::Unavailable).await;
    }

    /// Simulate slow network (add delay to all responses)
    pub async fn simulate_slow_network(&self, delay: Duration) {
        self.set_behavior(ServerBehavior::Slow { delay }).await;
    }

    /// Simulate intermittent failures
    pub async fn simulate_intermittent_failures(&self, error_rate: f64) {
        self.set_behavior(ServerBehavior::Failing { error_rate })
            .await;
    }

    /// Restore normal behavior
    pub async fn restore_normal_behavior(&self) {
        self.set_behavior(ServerBehavior::Normal).await;
    }
}

impl Default for MockConfigurationServer {
    fn default() -> Self {
        Self::new()
    }
}

/// The actual service implementation
struct MockConfigurationService {
    behavior: Arc<RwLock<ServerBehavior>>,
    data: Arc<RwLock<MockData>>,
    request_count: Arc<Mutex<HashMap<String, u32>>>,
}

impl MockConfigurationService {
    async fn increment_request_count(&self, method: &str) {
        let mut counts = self.request_count.lock().await;
        *counts.entry(method.to_string()).or_insert(0) += 1;
    }

    async fn should_fail(&self, method: &str) -> Option<ConfigurationApiError> {
        let behavior = self.behavior.read().await;

        match &*behavior {
            ServerBehavior::Normal => None,
            ServerBehavior::Slow { delay } => {
                sleep(*delay).await;
                None
            }
            ServerBehavior::Failing { error_rate } => {
                if rand::random::<f64>() < *error_rate {
                    Some(ConfigurationApiError::Unknown)
                } else {
                    None
                }
            }
            ServerBehavior::Unavailable => {
                // Simulate timeout by never responding
                sleep(Duration::from_secs(3600)).await;
                None
            }
            ServerBehavior::Custom {
                listener_behavior,
                route_behavior,
                backend_behavior,
                shared_filter_behavior,
            } => {
                let method_behavior = match method {
                    "listener" => listener_behavior,
                    "route" => route_behavior,
                    "backend" => backend_behavior,
                    "shared_filter" => shared_filter_behavior,
                    _ => &None,
                };

                if let Some(behavior) = method_behavior {
                    match behavior {
                        MethodBehavior::Normal => None,
                        MethodBehavior::Slow { delay } => {
                            sleep(*delay).await;
                            None
                        }
                        MethodBehavior::Error { error } => Some(*error),
                        MethodBehavior::Timeout => {
                            sleep(Duration::from_secs(3600)).await;
                            None
                        }
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[async_trait]
impl vg_rpc::ConfigurationApiServer for MockConfigurationService {
    async fn events(
        &self,
        _sink: PendingSubscriptionSink,
        _req: SubscribeEventsRequest,
    ) -> SubscriptionResult {
        self.increment_request_count("events").await;

        if let Some(error) = self.should_fail("events").await {
            return Err(jsonrpsee::core::SubscriptionError::from(format!(
                "Mock error: {:?}",
                error
            )));
        }

        // For testing, we'll create a simple subscription that doesn't send events
        // In a real implementation, this would manage event subscriptions
        Ok(())
    }

    async fn listener(&self, req: GetListenerRequest) -> Result<Listener, ConfigurationApiError> {
        self.increment_request_count("listener").await;

        if let Some(error) = self.should_fail("listener").await {
            return Err(error);
        }

        let data = self.data.read().await;
        data.listeners
            .get(&req.listener_ref())
            .cloned()
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn route(&self, req: GetRouteRequest) -> Result<Route, ConfigurationApiError> {
        self.increment_request_count("route").await;

        if let Some(error) = self.should_fail("route").await {
            return Err(error);
        }

        let data = self.data.read().await;
        data.routes
            .get(&req.route_ref())
            .cloned()
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn backend(&self, req: GetBackendRequest) -> Result<Backend, ConfigurationApiError> {
        self.increment_request_count("backend").await;

        if let Some(error) = self.should_fail("backend").await {
            return Err(error);
        }

        let data = self.data.read().await;
        data.backends
            .get(&req.backend_ref())
            .cloned()
            .ok_or(ConfigurationApiError::NotFound)
    }

    async fn shared_filter(
        &self,
        req: GetSharedFilterRequest,
    ) -> Result<SharedFilter, ConfigurationApiError> {
        self.increment_request_count("shared_filter").await;

        if let Some(error) = self.should_fail("shared_filter").await {
            return Err(error);
        }

        let data = self.data.read().await;
        data.shared_filters
            .get(&req.filter_ref())
            .cloned()
            .ok_or(ConfigurationApiError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_mock_server_creation() {
        let server = MockConfigurationServer::new();
        assert_eq!(server.get_request_count("listener").await, 0);
    }

    #[tokio::test]
    async fn test_mock_server_behavior_change() {
        let server = MockConfigurationServer::new();

        // Start with normal behavior
        server.set_behavior(ServerBehavior::Normal).await;

        // Change to slow behavior
        server
            .simulate_slow_network(Duration::from_millis(100))
            .await;

        // Change to failing behavior
        server.simulate_intermittent_failures(0.5).await;

        // Restore normal behavior
        server.restore_normal_behavior().await;
    }

    #[tokio::test]
    async fn test_mock_data_management() {
        let server = MockConfigurationServer::new();

        // Create test data
        let listener_ref = ListenerRef::from("test-listener".to_string());
        let listener = Listener::builder()
            .ref_(listener_ref.clone())
            .policies(vg_config::http::listener::policy::ListenerPolicies::default())
            .filters(Vec::new())
            .shared_filter_refs(Vec::new())
            .route_refs(Vec::new())
            .backend_refs(Vec::new())
            .build();

        // Add data to server
        server
            .add_listener(listener_ref.clone(), listener.clone())
            .await;

        // Verify data was added
        let data = server.data.read().await;
        assert!(data.listeners.contains_key(&listener_ref));
    }

    #[tokio::test]
    async fn test_request_counting() {
        let server = MockConfigurationServer::new();

        // Simulate some requests
        let service = MockConfigurationService {
            behavior: server.behavior.clone(),
            data: server.data.clone(),
            request_count: server.request_count.clone(),
        };

        service.increment_request_count("listener").await;
        service.increment_request_count("listener").await;
        service.increment_request_count("route").await;

        assert_eq!(server.get_request_count("listener").await, 2);
        assert_eq!(server.get_request_count("route").await, 1);
        assert_eq!(server.get_request_count("backend").await, 0);

        server.reset_request_counts().await;
        assert_eq!(server.get_request_count("listener").await, 0);
    }
}
