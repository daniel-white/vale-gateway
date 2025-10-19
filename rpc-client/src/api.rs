use crate::ConfigurationTransport;
use crate::instrumentation::TRACER;
use jsonrpsee::core::ClientError;
use opentelemetry::trace::{SpanKind, Tracer};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ConfigurationApiClient, ConfigurationApiError, GetBackendRequest, GetListenerRequest,
    GetRouteRequest, GetSharedFilterRequest, RequestContext,
};

#[derive(Clone, TypedBuilder)]
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Arc<Listener>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let client = self.transport.client();
        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport.listener_ref())
            .build();

        let listener = client.listener(req).await?;

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
        let client = self.transport.client();
        let req = GetRouteRequest::builder()
            .context(RequestContext::new(span))
            .route_ref(route_ref.clone())
            .build();

        let route = client.route(req).await?;

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
        let client = self.transport.client();
        let req = GetBackendRequest::builder()
            .context(RequestContext::new(span))
            .backend_ref(backend_ref.clone())
            .build();

        let backend = client.backend(req).await?;

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
        let client = self.transport.client();
        let req = GetSharedFilterRequest::builder()
            .context(RequestContext::new(span))
            .filter_ref(filter_ref.clone())
            .build();

        let filter = client.shared_filter(req).await?;

        Ok(Arc::new(filter))
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
