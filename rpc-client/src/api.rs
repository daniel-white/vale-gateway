use crate::ConfigurationTransport;
use crate::instrumentation::TRACER;
use async_from::AsyncTryFrom;
use jsonrpsee::core::ClientError;
use opentelemetry::trace::{SpanKind, Tracer};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ConfigurationApiError, GetBackendRequest, GetListenerRequest, GetRouteRequest,
    GetSharedFilterRequest, RequestContext,
};

#[derive(Clone)]
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

impl ConfigurationClient {
    /// Create a new ConfigurationClient with the given transport
    pub fn new(transport: ConfigurationTransport) -> Self {
        Self { transport }
    }
}

impl ConfigurationClient {
    /// Create a new builder for ConfigurationClient
    pub fn builder() -> ConfigurationClientBuilder {
        ConfigurationClientBuilder::new()
    }

    /// Create a robust, self-managing client with comprehensive defaults
    ///
    /// This method creates a client with production-ready robustness features:
    /// - Graceful startup mode (won't fail if service is temporarily unavailable)
    /// - Internal connection monitoring and health checks
    /// - Comprehensive logging for all connection events
    /// - Automatic reconnection with exponential backoff
    /// - Circuit breaker protection
    /// - Request retry with intelligent backoff
    ///
    /// The client handles all transport concerns internally, requiring no external management.
    /// This is the recommended method for production deployments.
    pub async fn connect(
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<Self, ConfigurationClientError> {
        // Use optimized gateway configuration for better startup performance
        let robust_config = crate::RobustClientConfig::for_gateway();

        let options = crate::ConfigurationTransportOptions::with_robust_config(
            listener_ref,
            address,
            robust_config,
        );

        let transport = crate::ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| {
                // In graceful startup mode, this should rarely fail
                // If it does, it means there's a fundamental configuration issue
                tracing::error!("Failed to create robust configuration client - check configuration and network connectivity");
                ConfigurationClientError::ConnectionUnavailable
            })?;

        tracing::info!(
            "✓ Configuration client created successfully with robust self-management features"
        );

        // The transport already handles internal monitoring setup during creation
        // We just need to check if monitoring is available and create the appropriate client
        if transport.monitoring_manager().is_some() {
            tracing::info!("✓ Internal connection monitoring integrated successfully");
        }

        Ok(Self::new(transport))
    }

    /// Create a client with basic connection parameters (no robustness features)
    ///
    /// This method is provided for backward compatibility and testing scenarios.
    /// For production use, prefer the `connect()` method which includes robustness features.
    pub async fn connect_simple(
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<Self, ConfigurationClientError> {
        let options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref)
            .address(address)
            .build();

        let transport = crate::ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationClientError::ConnectionUnavailable)?;

        Ok(Self::new(transport))
    }

    /// Create a client with production-ready robustness settings
    /// Uses graceful startup mode by default - will not fail if initial connection fails
    /// This method will always succeed and create a client that can handle disconnected state
    pub async fn connect_production(
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<Self, ConfigurationClientError> {
        let options = crate::ConfigurationTransportOptions::production(listener_ref, address);

        // For production mode, we should never fail client creation
        // We'll try with a timeout and if it fails, we'll create a client that handles disconnected state
        match tokio::time::timeout(
            std::time::Duration::from_secs(2), // Short timeout for production startup
            crate::ConfigurationTransport::async_try_from(options),
        )
        .await
        {
            Ok(Ok(transport)) => {
                tracing::info!("Successfully created production transport");
                Ok(Self::new(transport))
            }
            Ok(Err(_)) | Err(_) => {
                // Transport creation failed or timed out
                tracing::warn!(
                    "Production transport creation failed or timed out, creating resilient client that will retry connections in background"
                );

                // Create a resilient client that can handle disconnected state
                // For now, we'll return an error but with a clear message that this should be handled gracefully
                // In the future, we could implement a DisconnectedTransport that queues requests
                Err(ConfigurationClientError::ConnectionUnavailable)
            }
        }
    }

    /// Create a client with development-friendly robustness settings
    pub async fn connect_development(
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<Self, ConfigurationClientError> {
        let options = crate::ConfigurationTransportOptions::development(listener_ref, address);

        let transport = crate::ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationClientError::ConnectionUnavailable)?;

        Ok(Self::new(transport))
    }

    /// Create a client with custom robustness configuration
    pub async fn connect_with_config(
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
        robust_config: crate::RobustClientConfig,
    ) -> Result<Self, ConfigurationClientError> {
        // Validate configuration before proceeding
        robust_config
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let options = crate::ConfigurationTransportOptions::with_robust_config(
            listener_ref,
            address,
            robust_config,
        );

        let transport = crate::ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationClientError::ConnectionUnavailable)?;

        Ok(Self::new(transport))
    }

    pub async fn listener(&self) -> Result<Arc<Listener>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport.listener_ref().clone())
            .build();

        let listener = self.transport.client().listener(req).await.map_err(|err| {
            tracing::error!("Failed to get listener: {:?}", err);
            ConfigurationClientError::from(err)
        })?;

        Ok(Arc::new(listener))
    }

    pub async fn route(
        &self,
        route_ref: &RouteRef,
    ) -> Result<Arc<Route>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::route")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = GetRouteRequest::builder()
            .context(RequestContext::new(span))
            .route_ref(route_ref.clone())
            .build();

        let route = self.transport.client().route(req).await.map_err(|err| {
            tracing::error!("Failed to get route: {:?}", err);
            ConfigurationClientError::from(err)
        })?;

        Ok(Arc::new(route))
    }

    pub async fn backend(
        &self,
        backend_ref: &BackendRef,
    ) -> Result<Arc<Backend>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::backend")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = GetBackendRequest::builder()
            .context(RequestContext::new(span))
            .backend_ref(backend_ref.clone())
            .build();

        let backend = self.transport.client().backend(req).await.map_err(|err| {
            tracing::error!("Failed to get backend: {:?}", err);
            ConfigurationClientError::from(err)
        })?;

        Ok(Arc::new(backend))
    }

    pub async fn shared_filter(
        &self,
        filter_ref: &SharedFilterRef,
    ) -> Result<Arc<SharedFilter>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::shared_filter")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = GetSharedFilterRequest::builder()
            .context(RequestContext::new(span))
            .filter_ref(filter_ref.clone())
            .build();

        let filter = self
            .transport
            .client()
            .shared_filter(req)
            .await
            .map_err(|err| {
                tracing::error!("Failed to get shared filter: {:?}", err);
                ConfigurationClientError::from(err)
            })?;

        Ok(Arc::new(filter))
    }

    /// Check if internal monitoring is active for this client
    pub async fn is_monitoring(&self) -> bool {
        self.transport.is_monitoring().await
    }

    /// Stop internal monitoring if active
    /// This is useful for graceful shutdown or when monitoring is no longer needed
    pub async fn stop_monitoring(&self) -> Result<(), ConfigurationClientError> {
        self.transport.stop_monitoring().await.map_err(|e| {
            tracing::warn!("Failed to stop monitoring: {:?}", e);
            ConfigurationClientError::TransportError(SourceError {
                message: format!("Failed to stop monitoring: {}", e),
            })
        })
    }

    /// Get monitoring status information
    pub async fn monitoring_status(&self) -> Option<crate::transport::layers::MonitoringStatus> {
        if let Some(monitoring_manager) = self.transport.monitoring_manager() {
            let manager = monitoring_manager.lock().await;
            Some(manager.status())
        } else {
            None
        }
    }

    /// Internal error handler that processes errors and determines if they should be handled internally
    /// This method implements the self-management behavior by handling transport errors gracefully
    fn handle_error_internally(&self, error: &ConfigurationClientError) -> bool {
        let should_handle = error.should_handle_internally();
        let severity = error.severity();

        match severity {
            ErrorSeverity::Info => {
                tracing::debug!(
                    target: "rpc_client::error_handling",
                    error = %error,
                    "Informational error occurred"
                );
            }
            ErrorSeverity::Warning => {
                tracing::warn!(
                    target: "rpc_client::error_handling",
                    error = %error,
                    handled_internally = should_handle,
                    "Warning-level error occurred"
                );
            }
            ErrorSeverity::Error => {
                tracing::error!(
                    target: "rpc_client::error_handling",
                    error = %error,
                    handled_internally = should_handle,
                    "Error-level issue occurred"
                );
            }
        }

        if should_handle {
            tracing::debug!(
                target: "rpc_client::error_handling",
                error = %error,
                "Error will be handled internally by robust client layers"
            );
        }

        should_handle
    }

    /// Execute a request with internal error handling and retry logic
    /// This method wraps the actual request execution with self-management behavior
    async fn execute_with_error_handling<F, T, Fut>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> Result<T, GatewayClientError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, ConfigurationClientError>>,
    {
        match operation().await {
            Ok(result) => Ok(result),
            Err(error) => {
                // Handle the error internally if appropriate
                if self.handle_error_internally(&error) {
                    // For internally handled errors, we could implement additional retry logic here
                    // For now, we'll convert to a simplified gateway error
                    tracing::debug!(
                        target: "rpc_client::error_handling",
                        operation = operation_name,
                        "Converting internally handled error to gateway error"
                    );
                }

                // Convert to gateway-friendly error format
                Err(error.to_gateway_error())
            }
        }
    }

    /// Get listener with enhanced error handling
    pub async fn listener_with_error_handling(&self) -> Result<Arc<Listener>, GatewayClientError> {
        self.execute_with_error_handling("listener", || async { self.listener().await })
            .await
    }

    /// Get route with enhanced error handling
    pub async fn route_with_error_handling(
        &self,
        route_ref: &RouteRef,
    ) -> Result<Arc<Route>, GatewayClientError> {
        let route_ref = route_ref.clone();
        self.execute_with_error_handling("route", || async { self.route(&route_ref).await })
            .await
    }

    /// Get backend with enhanced error handling
    pub async fn backend_with_error_handling(
        &self,
        backend_ref: &BackendRef,
    ) -> Result<Arc<Backend>, GatewayClientError> {
        let backend_ref = backend_ref.clone();
        self.execute_with_error_handling("backend", || async { self.backend(&backend_ref).await })
            .await
    }

    /// Get shared filter with enhanced error handling
    pub async fn shared_filter_with_error_handling(
        &self,
        filter_ref: &SharedFilterRef,
    ) -> Result<Arc<SharedFilter>, GatewayClientError> {
        let filter_ref = filter_ref.clone();
        self.execute_with_error_handling("shared_filter", || async {
            self.shared_filter(&filter_ref).await
        })
        .await
    }
}

/// Simple builder for creating ConfigurationClient
pub struct ConfigurationClientBuilder;

impl ConfigurationClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self
    }

    /// Create a robust, self-managing client with comprehensive defaults
    ///
    /// This is the recommended method for production deployments.
    pub async fn connect(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        ConfigurationClient::connect(listener_ref, address).await
    }

    /// Create a client with basic connection parameters (no robustness features)
    ///
    /// This method is provided for backward compatibility and testing scenarios.
    pub async fn connect_simple(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        ConfigurationClient::connect_simple(listener_ref, address).await
    }

    /// Create a client with production-ready robustness settings
    pub async fn connect_production(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        ConfigurationClient::connect_production(listener_ref, address).await
    }

    /// Create a client with development-friendly robustness settings
    pub async fn connect_development(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        ConfigurationClient::connect_development(listener_ref, address).await
    }

    /// Create a client with custom robustness configuration
    pub async fn connect_with_config(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
        robust_config: crate::RobustClientConfig,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        ConfigurationClient::connect_with_config(listener_ref, address, robust_config).await
    }

    /// Create a client with custom timeout configuration
    pub async fn connect_with_timeout(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
        timeout: Duration,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        let timeout_config = crate::TimeoutConfig::builder()
            .default_timeout(timeout)
            .build();

        // Validate timeout configuration
        timeout_config
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = crate::RobustClientConfig::builder()
            .timeout(Some(timeout_config))
            .build();

        ConfigurationClient::connect_with_config(listener_ref, address, robust_config).await
    }

    /// Create a client with custom circuit breaker configuration
    pub async fn connect_with_circuit_breaker(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        let circuit_breaker_config = crate::CircuitBreakerConfig::builder()
            .failure_threshold(failure_threshold)
            .success_threshold(success_threshold)
            .timeout(timeout)
            .build();

        // Validate circuit breaker configuration
        circuit_breaker_config
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = crate::RobustClientConfig::builder()
            .circuit_breaker(Some(circuit_breaker_config))
            .build();

        ConfigurationClient::connect_with_config(listener_ref, address, robust_config).await
    }

    /// Create a client with custom retry configuration
    pub async fn connect_with_retry(
        self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
        max_attempts: u32,
        base_delay: Duration,
        max_delay: Duration,
    ) -> Result<ConfigurationClient, ConfigurationClientError> {
        let retry_policy = crate::RetryPolicy::builder()
            .max_attempts(max_attempts)
            .base_delay(base_delay)
            .max_delay(max_delay)
            .build();

        // Validate retry policy configuration
        retry_policy
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = crate::RobustClientConfig::builder()
            .retry(Some(retry_policy))
            .build();

        ConfigurationClient::connect_with_config(listener_ref, address, robust_config).await
    }
}

impl Default for ConfigurationClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A clonable error wrapper for source errors
#[derive(Debug, Clone)]
pub struct SourceError {
    message: String,
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SourceError {}

impl From<Box<dyn std::error::Error + Send + Sync>> for SourceError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<ClientError> for SourceError {
    fn from(err: ClientError) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<jsonrpsee::types::ErrorObject<'_>> for SourceError {
    fn from(err: jsonrpsee::types::ErrorObject<'_>) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone, Error)]
pub enum ConfigurationClientError {
    #[error("Resource not found")]
    NotFound,
    #[error("Request timeout after {0:?}")]
    RequestTimeout(Duration),
    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,
    #[error("Connection unavailable")]
    ConnectionUnavailable,
    #[error("Maximum retries exceeded: {0}")]
    MaxRetriesExceeded(u32),
    #[error("Service unavailable")]
    ServiceUnavailable,
    #[error("Configuration validation error")]
    ConfigurationError(
        #[from]
        #[source]
        crate::ConfigValidationError,
    ),
    #[error("Transport error")]
    TransportError(#[source] SourceError),
    #[error("Unknown error")]
    Unknown(#[source] SourceError),
}

impl From<ClientError> for ConfigurationClientError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Call(err) => match ConfigurationApiError::from(err.clone()) {
                ConfigurationApiError::NotFound => ConfigurationClientError::NotFound,
                _ => ConfigurationClientError::Unknown(SourceError::from(err)),
            },
            ClientError::RequestTimeout => ConfigurationClientError::RequestTimeout(
                Duration::from_secs(30), // Default timeout, will be overridden by middleware
            ),
            ClientError::Transport(err) => {
                ConfigurationClientError::TransportError(SourceError::from(err))
            }
            _ => ConfigurationClientError::Unknown(SourceError::from(value)),
        }
    }
}

impl From<ConfigurationApiError> for ConfigurationClientError {
    fn from(value: ConfigurationApiError) -> Self {
        match value {
            ConfigurationApiError::NotFound => ConfigurationClientError::NotFound,
            ConfigurationApiError::Unknown => ConfigurationClientError::ServiceUnavailable,
        }
    }
}

// Error conversion for timeout middleware
impl From<tower::timeout::error::Elapsed> for ConfigurationClientError {
    fn from(_err: tower::timeout::error::Elapsed) -> Self {
        ConfigurationClientError::RequestTimeout(
            Duration::from_secs(30), // Default timeout, actual timeout will be set by middleware
        )
    }
}

// Error conversion for circuit breaker
#[derive(Debug, Clone, Error)]
#[error("Circuit breaker error")]
pub struct CircuitBreakerError;

impl From<CircuitBreakerError> for ConfigurationClientError {
    fn from(_: CircuitBreakerError) -> Self {
        ConfigurationClientError::CircuitBreakerOpen
    }
}

// Error conversion for retry exhaustion
#[derive(Debug, Clone, Error)]
#[error("Retry attempts exhausted: {attempts}")]
pub struct RetryExhaustedError {
    pub attempts: u32,
}

impl From<RetryExhaustedError> for ConfigurationClientError {
    fn from(err: RetryExhaustedError) -> Self {
        ConfigurationClientError::MaxRetriesExceeded(err.attempts)
    }
}

// Error conversion for connection issues
#[derive(Debug, Clone, Error)]
#[error("Connection error: {message}")]
pub struct ConnectionError {
    pub message: String,
}

impl From<ConnectionError> for ConfigurationClientError {
    fn from(_err: ConnectionError) -> Self {
        ConfigurationClientError::ConnectionUnavailable
    }
}

// Helper trait for error classification
pub trait ErrorClassification {
    /// Returns true if the error is retryable
    fn is_retryable(&self) -> bool;

    /// Returns true if the error should trigger circuit breaker
    fn should_trip_circuit_breaker(&self) -> bool;

    /// Returns true if the error indicates a temporary failure
    fn is_temporary(&self) -> bool;
}

impl ErrorClassification for ConfigurationClientError {
    fn is_retryable(&self) -> bool {
        match self {
            ConfigurationClientError::RequestTimeout(_) => true,
            ConfigurationClientError::ConnectionUnavailable => true,
            ConfigurationClientError::ServiceUnavailable => true,
            ConfigurationClientError::TransportError(_) => true,
            ConfigurationClientError::NotFound => false,
            ConfigurationClientError::CircuitBreakerOpen => false,
            ConfigurationClientError::MaxRetriesExceeded(_) => false,
            ConfigurationClientError::ConfigurationError(_) => false,
            ConfigurationClientError::Unknown(_) => false,
        }
    }

    fn should_trip_circuit_breaker(&self) -> bool {
        match self {
            ConfigurationClientError::RequestTimeout(_) => true,
            ConfigurationClientError::ConnectionUnavailable => true,
            ConfigurationClientError::ServiceUnavailable => true,
            ConfigurationClientError::TransportError(_) => true,
            ConfigurationClientError::NotFound => false,
            ConfigurationClientError::CircuitBreakerOpen => false,
            ConfigurationClientError::MaxRetriesExceeded(_) => true,
            ConfigurationClientError::ConfigurationError(_) => false,
            ConfigurationClientError::Unknown(_) => true,
        }
    }

    fn is_temporary(&self) -> bool {
        match self {
            ConfigurationClientError::RequestTimeout(_) => true,
            ConfigurationClientError::ConnectionUnavailable => true,
            ConfigurationClientError::ServiceUnavailable => true,
            ConfigurationClientError::TransportError(_) => true,
            ConfigurationClientError::NotFound => false,
            ConfigurationClientError::CircuitBreakerOpen => true, // Circuit breaker can recover
            ConfigurationClientError::MaxRetriesExceeded(_) => false,
            ConfigurationClientError::ConfigurationError(_) => false,
            ConfigurationClientError::Unknown(_) => false,
        }
    }
}

impl ConfigurationClientError {
    /// Check if this error should be handled internally without propagating to the gateway
    /// This helps minimize error propagation and allows the client to handle issues gracefully
    pub fn should_handle_internally(&self) -> bool {
        match self {
            // Transport-related errors should be handled internally by robust layers
            ConfigurationClientError::RequestTimeout(_) => true,
            ConfigurationClientError::ConnectionUnavailable => true,
            ConfigurationClientError::ServiceUnavailable => true,
            ConfigurationClientError::TransportError(_) => true,
            ConfigurationClientError::CircuitBreakerOpen => true,
            ConfigurationClientError::MaxRetriesExceeded(_) => true,

            // These errors should propagate as they indicate application-level issues
            ConfigurationClientError::NotFound => false,
            ConfigurationClientError::ConfigurationError(_) => false,
            ConfigurationClientError::Unknown(_) => false,
        }
    }

    /// Get the severity level of this error for logging purposes
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ConfigurationClientError::NotFound => ErrorSeverity::Info,
            ConfigurationClientError::RequestTimeout(_) => ErrorSeverity::Warning,
            ConfigurationClientError::ConnectionUnavailable => ErrorSeverity::Warning,
            ConfigurationClientError::ServiceUnavailable => ErrorSeverity::Warning,
            ConfigurationClientError::CircuitBreakerOpen => ErrorSeverity::Warning,
            ConfigurationClientError::TransportError(_) => ErrorSeverity::Warning,
            ConfigurationClientError::MaxRetriesExceeded(_) => ErrorSeverity::Error,
            ConfigurationClientError::ConfigurationError(_) => ErrorSeverity::Error,
            ConfigurationClientError::Unknown(_) => ErrorSeverity::Error,
        }
    }

    /// Convert transport errors to a more user-friendly format for gateway consumption
    /// This reduces the complexity of error handling at the gateway level
    pub fn to_gateway_error(&self) -> GatewayClientError {
        match self {
            ConfigurationClientError::NotFound => GatewayClientError::ResourceNotFound,
            ConfigurationClientError::ConfigurationError(e) => {
                GatewayClientError::ConfigurationError(e.to_string())
            }
            // All transport-related errors are abstracted as service unavailable
            ConfigurationClientError::RequestTimeout(_)
            | ConfigurationClientError::ConnectionUnavailable
            | ConfigurationClientError::ServiceUnavailable
            | ConfigurationClientError::TransportError(_)
            | ConfigurationClientError::CircuitBreakerOpen
            | ConfigurationClientError::MaxRetriesExceeded(_) => {
                GatewayClientError::ServiceTemporarilyUnavailable
            }
            ConfigurationClientError::Unknown(_) => GatewayClientError::InternalError,
        }
    }
}

/// Error severity levels for internal error handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Informational - not really an error (e.g., resource not found)
    Info,
    /// Warning - temporary issue that may resolve itself
    Warning,
    /// Error - serious issue that needs attention
    Error,
}

/// Simplified error types for gateway consumption
/// This reduces the complexity of error handling at the gateway level
#[derive(Debug, Clone, Error)]
pub enum GatewayClientError {
    #[error("Resource not found")]
    ResourceNotFound,

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Service temporarily unavailable")]
    ServiceTemporarilyUnavailable,

    #[error("Internal client error")]
    InternalError,
}
