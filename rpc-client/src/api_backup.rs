use crate::ConfigurationTransport;
use crate::instrumentation::TRACER;
use async_from::AsyncTryFrom;
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
    ConfigurationApiError, GetBackendRequest, GetListenerRequest, GetRouteRequest,
    GetSharedFilterRequest, RequestContext,
};

#[derive(Clone, TypedBuilder)]
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

/// Builder for creating ConfigurationClient with convenient configuration methods
pub struct ConfigurationClientBuilder;

impl ConfigurationClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self
    }

    /// Set the transport options directly
    pub fn transport_options(mut self, options: crate::ConfigurationTransportOptions) -> Self {
        self.transport_options = Some(options);
        self
    }

    /// Configure basic connection parameters
    pub fn connection(
        mut self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Self {
        let options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref)
            .address(address)
            .build();
        self.transport_options = Some(options);
        self
    }

    /// Configure connection with production-ready robustness settings
    pub fn production_ready(
        mut self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Self {
        let options = crate::ConfigurationTransportOptions::production(listener_ref, address);
        self.transport_options = Some(options);
        self
    }

    /// Configure connection with development-friendly robustness settings
    pub fn development_ready(
        mut self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Self {
        let options = crate::ConfigurationTransportOptions::development(listener_ref, address);
        self.transport_options = Some(options);
        self
    }

    /// Configure connection with custom robustness settings
    pub fn with_robustness(
        mut self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
        robust_config: crate::RobustClientConfig,
    ) -> Self {
        let options = crate::ConfigurationTransportOptions::with_robust_config(
            listener_ref,
            address,
            robust_config,
        );
        self.transport_options = Some(options);
        self
    }

    /// Configure connection with default robustness settings
    pub fn with_default_robustness(
        mut self,
        listener_ref: impl Into<vg_config::http::listener::ListenerRef>,
        address: impl Into<http::Uri>,
    ) -> Self {
        let options =
            crate::ConfigurationTransportOptions::with_default_robustness(listener_ref, address);
        self.transport_options = Some(options);
        self
    }

    /// Configure timeout settings
    pub fn with_timeout(
        mut self,
        default_timeout: Duration,
    ) -> Result<Self, ConfigurationClientError> {
        let mut options = self.transport_options.take().ok_or_else(|| {
            ConfigurationClientError::ConfigurationError(
                crate::ConfigValidationError::InvalidTimeout(
                    "Connection must be configured before timeout settings".to_string(),
                ),
            )
        })?;

        let timeout_config = crate::TimeoutConfig::builder()
            .default_timeout(default_timeout)
            .build();

        // Validate timeout configuration
        timeout_config
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = options.robust_config().cloned().unwrap_or_default();

        let updated_config = crate::RobustClientConfig::builder()
            .timeout(Some(timeout_config))
            .circuit_breaker(robust_config.circuit_breaker)
            .retry(robust_config.retry)
            .reconnection(robust_config.reconnection)
            .instrumentation(robust_config.instrumentation)
            .build();

        // Update the options with the new robust config
        options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(options.listener_ref.clone())
            .address(options.address.clone())
            .robust_config(Some(updated_config))
            .build();

        self.transport_options = Some(options);
        Ok(self)
    }

    /// Configure circuit breaker settings
    pub fn with_circuit_breaker(
        mut self,
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Result<Self, ConfigurationClientError> {
        let mut options = self.transport_options.take().ok_or_else(|| {
            ConfigurationClientError::ConfigurationError(
                crate::ConfigValidationError::InvalidCircuitBreaker(
                    "Connection must be configured before circuit breaker settings".to_string(),
                ),
            )
        })?;

        let circuit_breaker_config = crate::CircuitBreakerConfig::builder()
            .failure_threshold(failure_threshold)
            .success_threshold(success_threshold)
            .timeout(timeout)
            .build();

        // Validate circuit breaker configuration
        circuit_breaker_config
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = options.robust_config().cloned().unwrap_or_default();

        let updated_config = crate::RobustClientConfig::builder()
            .timeout(robust_config.timeout)
            .circuit_breaker(Some(circuit_breaker_config))
            .retry(robust_config.retry)
            .reconnection(robust_config.reconnection)
            .instrumentation(robust_config.instrumentation)
            .build();

        // Update the options with the new robust config
        options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(options.listener_ref.clone())
            .address(options.address.clone())
            .robust_config(Some(updated_config))
            .build();

        self.transport_options = Some(options);
        Ok(self)
    }

    /// Configure retry settings
    pub fn with_retry(
        mut self,
        max_attempts: u32,
        base_delay: Duration,
        max_delay: Duration,
    ) -> Result<Self, ConfigurationClientError> {
        let mut options = self.transport_options.take().ok_or_else(|| {
            ConfigurationClientError::ConfigurationError(
                crate::ConfigValidationError::InvalidRetryPolicy(
                    "Connection must be configured before retry settings".to_string(),
                ),
            )
        })?;

        let retry_policy = crate::RetryPolicy::builder()
            .max_attempts(max_attempts)
            .base_delay(base_delay)
            .max_delay(max_delay)
            .build();

        // Validate retry policy configuration
        retry_policy
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = options.robust_config().cloned().unwrap_or_default();

        let updated_config = crate::RobustClientConfig::builder()
            .timeout(robust_config.timeout)
            .circuit_breaker(robust_config.circuit_breaker)
            .retry(Some(retry_policy))
            .reconnection(robust_config.reconnection)
            .instrumentation(robust_config.instrumentation)
            .build();

        // Update the options with the new robust config
        options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(options.listener_ref.clone())
            .address(options.address.clone())
            .robust_config(Some(updated_config))
            .build();

        self.transport_options = Some(options);
        Ok(self)
    }

    /// Configure reconnection settings
    pub fn with_reconnection(
        mut self,
        enable_lazy_connection: bool,
        max_reconnect_attempts: Option<u32>,
        reconnect_base_delay: Duration,
    ) -> Result<Self, ConfigurationClientError> {
        let mut options = self.transport_options.take().ok_or_else(|| {
            ConfigurationClientError::ConfigurationError(
                crate::ConfigValidationError::InvalidReconnection(
                    "Connection must be configured before reconnection settings".to_string(),
                ),
            )
        })?;

        let reconnection_config = crate::ReconnectionConfig::builder()
            .enable_lazy_connection(enable_lazy_connection)
            .max_reconnect_attempts(max_reconnect_attempts)
            .reconnect_base_delay(reconnect_base_delay)
            .build();

        // Validate reconnection configuration
        reconnection_config
            .validate()
            .map_err(ConfigurationClientError::ConfigurationError)?;

        let robust_config = options.robust_config().cloned().unwrap_or_default();

        let updated_config = crate::RobustClientConfig::builder()
            .timeout(robust_config.timeout)
            .circuit_breaker(robust_config.circuit_breaker)
            .retry(robust_config.retry)
            .reconnection(Some(reconnection_config))
            .instrumentation(robust_config.instrumentation)
            .build();

        // Update the options with the new robust config
        options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(options.listener_ref.clone())
            .address(options.address.clone())
            .robust_config(Some(updated_config))
            .build();

        self.transport_options = Some(options);
        Ok(self)
    }

    /// Enable or disable instrumentation features
    pub fn with_instrumentation(
        mut self,
        enable_metrics: bool,
        enable_tracing: bool,
        enable_logging: bool,
    ) -> Result<Self, ConfigurationClientError> {
        let mut options = self.transport_options.take().ok_or_else(|| {
            ConfigurationClientError::ConfigurationError(
                crate::ConfigValidationError::InvalidTimeout(
                    "Connection must be configured before instrumentation settings".to_string(),
                ),
            )
        })?;

        let instrumentation_config = crate::InstrumentationConfig::builder()
            .enable_metrics(enable_metrics)
            .enable_tracing(enable_tracing)
            .enable_logging(enable_logging)
            .build();

        let robust_config = options.robust_config().cloned().unwrap_or_default();

        let updated_config = crate::RobustClientConfig::builder()
            .timeout(robust_config.timeout)
            .circuit_breaker(robust_config.circuit_breaker)
            .retry(robust_config.retry)
            .reconnection(robust_config.reconnection)
            .instrumentation(instrumentation_config)
            .build();

        // Update the options with the new robust config
        options = crate::ConfigurationTransportOptions::builder()
            .listener_ref(options.listener_ref.clone())
            .address(options.address.clone())
            .robust_config(Some(updated_config))
            .build();

        self.transport_options = Some(options);
        Ok(self)
    }

    /// Build the ConfigurationClient
    pub async fn build(self) -> Result<ConfigurationClient, ConfigurationClientError> {
        let options = self.transport_options.ok_or_else(|| {
            ConfigurationClientError::ConfigurationError(
                crate::ConfigValidationError::InvalidTimeout(
                    "Connection must be configured before building client".to_string(),
                ),
            )
        })?;

        // Validate the complete configuration if robustness is enabled
        if let Some(robust_config) = options.robust_config() {
            robust_config
                .validate()
                .map_err(ConfigurationClientError::ConfigurationError)?;
        }

        let transport = crate::ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationClientError::ConnectionUnavailable)?;

        Ok(ConfigurationClient::builder().transport(transport).build())
    }
}

impl Default for ConfigurationClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationClient {
    /// Create a new builder for ConfigurationClient
    pub fn builder() -> ConfigurationClientBuilder {
        ConfigurationClientBuilder::new()
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
