use crate::instrumentation::TRACER;
use crate::transport::rpc::{RpcTransport, RpcTransportError};

use getset::Getters;
use jsonrpsee::core::ClientError;
use opentelemetry::trace::{SpanKind, Tracer};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;

use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ConfigurationApiClient, ConfigurationApiError, GetBackendRequest, GetListenerRequest,
    GetRouteRequest, GetSharedFilterRequest, RequestContext,
};

/// Configuration client that uses RPC transport
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct ConfigurationClient {
    /// RPC transport instance
    #[getset(get = "pub")]
    transport: RpcTransport,

    /// Listener reference for this client
    #[getset(get = "pub")]
    listener_ref: ListenerRef,
}

impl ConfigurationClient {
    /// Create a new ConfigurationClient with RPC transport
    /// This replaces the connect() method - transport is now passed in
    pub fn new(transport: RpcTransport, listener_ref: impl Into<ListenerRef>) -> Self {
        Self::builder()
            .transport(transport)
            .listener_ref(listener_ref.into())
            .build()
    }
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Arc<Listener>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.listener_ref().clone())
            .build();

        let client = self.transport().current_client().await;
        let listener = client.listener(req).await.map_err(|err| {
            tracing::error!("Failed to get listener: {:?}", err);
            
            // Check if this is a connection-related error that might indicate stale client
            match &err {
                ClientError::Transport(_) | ClientError::RequestTimeout => {
                    tracing::warn!("Listener request failed with transport/timeout error, connection may be stale");
                }
                _ => {}
            }
            
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

        let client = self.transport().current_client().await;
        let route = client.route(req).await.map_err(|err| {
            tracing::error!("Failed to get route: {:?}", err);
            
            // Check if this is a connection-related error that might indicate stale client
            match &err {
                ClientError::Transport(_) | ClientError::RequestTimeout => {
                    tracing::warn!("Route request failed with transport/timeout error, connection may be stale");
                }
                _ => {}
            }
            
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

        let client = self.transport().current_client().await;
        let backend = client.backend(req).await.map_err(|err| {
            tracing::error!("Failed to get backend: {:?}", err);
            
            // Check if this is a connection-related error that might indicate stale client
            match &err {
                ClientError::Transport(_) | ClientError::RequestTimeout => {
                    tracing::warn!("Backend request failed with transport/timeout error, connection may be stale");
                }
                _ => {}
            }
            
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

        let client = self.transport().current_client().await;
        let filter = client.shared_filter(req).await.map_err(|err| {
            tracing::error!("Failed to get shared filter: {:?}", err);
            
            // Check if this is a connection-related error that might indicate stale client
            match &err {
                ClientError::Transport(_) | ClientError::RequestTimeout => {
                    tracing::warn!("Shared filter request failed with transport/timeout error, connection may be stale");
                }
                _ => {}
            }
            
            ConfigurationClientError::from(err)
        })?;

        Ok(Arc::new(filter))
    }

    /// Check if internal monitoring is active for this client
    pub async fn is_monitoring(&self) -> bool {
        self.transport().is_monitoring().await
    }

    /// Stop internal monitoring if active
    /// This is useful for graceful shutdown or when monitoring is no longer needed
    pub async fn stop_monitoring(&self) -> Result<(), ConfigurationClientError> {
        self.transport().stop_monitoring().await.map_err(|e| {
            tracing::warn!("Failed to stop monitoring: {:?}", e);
            ConfigurationClientError::TransportError(SourceError {
                message: format!("Failed to stop monitoring: {}", e),
            })
        })
    }

    /// Get monitoring status information
    pub async fn monitoring_status(&self) -> Option<crate::transport::layers::MonitoringStatus> {
        Some(self.transport().monitoring_status().await)
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

// Error conversion for RPC transport
impl From<RpcTransportError> for ConfigurationClientError {
    fn from(err: RpcTransportError) -> Self {
        match err {
            RpcTransportError::InitializationFailed(_) => {
                ConfigurationClientError::ConnectionUnavailable
            }
            RpcTransportError::ConfigurationMismatch => {
                ConfigurationClientError::ConfigurationError(
                    crate::ConfigValidationError::InvalidInternalMonitoring(
                        "RPC transport configuration mismatch".to_string(),
                    ),
                )
            }
            RpcTransportError::MonitoringError(_) => {
                ConfigurationClientError::TransportError(SourceError {
                    message: err.to_string(),
                })
            }
        }
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
