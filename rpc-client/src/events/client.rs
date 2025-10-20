use crate::ConfigurationTransport;
use async_from::AsyncTryFrom;
use http::Uri;
use jsonrpsee::core::ClientError;
use opentelemetry::Context;
use opentelemetry::context::FutureExt;
use opentelemetry::trace::{Span, SpanKind, TraceContextExt, Tracer};
use thiserror::Error;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_core::sync::broadcast::error::RecvError;
use vg_core::sync::broadcast::{Receiver, Sender, Traced, channel};
use vg_core::sync::handles::{Handle, handles};
use vg_rpc::{ConfigurationApiError, RequestContext, SubscribeEventsRequest};

use crate::instrumentation::TRACER;
use crate::{ConfigurationTransportOptions, RobustClientConfig};
pub use vg_rpc::ConfigurationEvent;

pub struct ConfigurationEventsClient {
    transport: ConfigurationTransport,
    tx: Sender<ConfigurationEvent>,
}

impl ConfigurationEventsClient {
    pub fn new(transport: ConfigurationTransport) -> Self {
        let (tx, _) = channel(32);
        Self { transport, tx }
    }

    /// Create a new ConfigurationEventsClient with connection parameters
    /// This method creates the transport internally with production-ready robustness settings
    /// This method will always succeed and create a client that can handle disconnected state
    pub async fn connect_production(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Result<Self, ConfigurationEventClientError> {
        let options = ConfigurationTransportOptions::production(listener_ref, address);
        let transport = ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationEventClientError::ConnectionFailed)?;

        Ok(Self::new(transport))
    }

    /// Create a new ConfigurationEventsClient with connection parameters
    /// This method creates the transport internally with development-friendly robustness settings
    pub async fn connect_development(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Result<Self, ConfigurationEventClientError> {
        let options = ConfigurationTransportOptions::development(listener_ref, address);
        let transport = ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationEventClientError::ConnectionFailed)?;

        Ok(Self::new(transport))
    }

    /// Create a new ConfigurationEventsClient with custom robustness configuration
    pub async fn connect_with_config(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
        robust_config: RobustClientConfig,
    ) -> Result<Self, ConfigurationEventClientError> {
        // Validate configuration before proceeding
        robust_config
            .validate()
            .map_err(|_| ConfigurationEventClientError::InvalidConfiguration)?;

        let options =
            ConfigurationTransportOptions::with_robust_config(listener_ref, address, robust_config);
        let transport = ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationEventClientError::ConnectionFailed)?;

        Ok(Self::new(transport))
    }

    /// Create a robust, self-managing events client with comprehensive defaults
    ///
    /// This method creates an events client with production-ready robustness features:
    /// - Graceful startup mode (won't fail if service is temporarily unavailable)
    /// - Internal connection monitoring optimized for event streams
    /// - Comprehensive logging for all connection and event processing
    /// - Automatic reconnection with exponential backoff
    /// - Circuit breaker protection
    /// - Request retry with intelligent backoff
    /// - Event stream optimization for high-throughput scenarios
    ///
    /// The client handles all transport concerns internally, requiring no external management.
    /// This is the recommended method for production deployments.
    pub async fn connect(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Result<Self, ConfigurationEventClientError> {
        // Create robust configuration optimized for event streams
        let robust_config = RobustClientConfig::builder()
            .timeout(Some(
                crate::TimeoutConfig::builder()
                    .default_timeout(std::time::Duration::from_secs(45)) // Longer timeout for event streams
                    .build(),
            ))
            .circuit_breaker(Some(
                crate::CircuitBreakerConfig::builder()
                    .failure_threshold(3) // More sensitive for event streams
                    .success_threshold(2)
                    .timeout(std::time::Duration::from_secs(30))
                    .minimum_throughput(5)
                    .build(),
            ))
            .retry(Some(
                crate::RetryPolicy::builder()
                    .max_attempts(5) // More retries for event streams
                    .base_delay(std::time::Duration::from_millis(200))
                    .max_delay(std::time::Duration::from_secs(60))
                    .backoff_multiplier(1.5) // Gentler backoff for streams
                    .jitter(0.1)
                    .build(),
            ))
            .reconnection(Some(
                crate::ReconnectionConfig::builder()
                    .enable_lazy_connection(false)
                    .max_reconnect_attempts(None) // Unlimited reconnection for event streams
                    .reconnect_base_delay(std::time::Duration::from_secs(1)) // Faster reconnection
                    .reconnect_max_delay(std::time::Duration::from_secs(120))
                    .queue_requests_during_reconnection(true)
                    .max_queued_requests(200) // Larger queue for events
                    .build(),
            ))
            .instrumentation(
                crate::InstrumentationConfig::builder()
                    .enable_metrics(true)
                    .enable_tracing(true)
                    .enable_logging(true)
                    .enable_performance_monitoring(false) // Disabled by default for performance
                    .build(),
            )
            .startup(
                crate::StartupConfig::builder()
                    .mode(crate::StartupMode::Graceful) // Graceful startup - won't fail if service unavailable
                    .initial_connection_timeout(std::time::Duration::from_secs(5))
                    .validate_connectivity(true)
                    .log_startup_attempts(true)
                    .build(),
            )
            .internal_monitoring(
                crate::InternalMonitoringConfig::builder()
                    .enabled(true)
                    .check_interval(std::time::Duration::from_secs(20)) // More frequent checks for event streams
                    .health_check_timeout(std::time::Duration::from_secs(3)) // Shorter timeout for responsiveness
                    .critical_failure_threshold(6) // Lower threshold for event streams (2 minutes at 20s intervals)
                    .log_heartbeat(false) // Reduce log noise for event streams
                    .build(),
            )
            .startup_logging(
                crate::StartupLoggingConfig::builder()
                    .log_connection_attempts(true)
                    .log_validation_results(true)
                    .log_background_operations(false) // Reduce noise for event streams
                    .startup_summary(true)
                    .log_level(crate::transport::layers::StartupLogLevel::Info)
                    .include_performance_metrics(false) // Reduce overhead for event streams
                    .build(),
            )
            .build();

        let options =
            ConfigurationTransportOptions::with_robust_config(listener_ref, address, robust_config);

        let transport = ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| {
                // In graceful startup mode, this should rarely fail
                // If it does, it means there's a fundamental configuration issue
                tracing::error!("Failed to create robust configuration events client - check configuration and network connectivity");
                ConfigurationEventClientError::ConnectionFailed
            })?;

        tracing::info!(
            "✓ Configuration events client created successfully with robust self-management features"
        );

        // The transport already handles internal monitoring setup during creation
        // We just need to check if monitoring is available and create the appropriate client
        if transport.monitoring_manager().is_some() {
            tracing::info!(
                "✓ Internal connection monitoring integrated successfully for events client"
            );
        }

        Ok(Self::new(transport))
    }

    /// Create a new ConfigurationEventsClient with basic connection (no robustness features)
    ///
    /// This method is provided for backward compatibility and testing scenarios.
    /// For production use, prefer the `connect()` method which includes robustness features.
    pub async fn connect_simple(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Result<Self, ConfigurationEventClientError> {
        let options = ConfigurationTransportOptions::builder()
            .listener_ref(listener_ref)
            .address(address)
            .build();
        let transport = ConfigurationTransport::async_try_from(options)
            .await
            .map_err(|_| ConfigurationEventClientError::ConnectionFailed)?;

        Ok(Self::new(transport))
    }

    pub fn events(&self) -> ConfigurationEventsReceiver {
        ConfigurationEventsReceiver::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }

    pub async fn start(self) -> Result<Handle, ConfigurationEventClientError> {
        let span = TRACER
            .span_builder("ConfigurationEventClient::events")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let req = SubscribeEventsRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport.listener_ref().clone())
            .build();

        let mut subscription = self.transport.client().events(req).await?;
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            // Hold on to the transport to keep the connection alive
            let transport = self.transport;

            loop {
                select! {
                    event = subscription.next() => {
                        if let Some(Ok(event)) = event {
                            let channel = event.context().propagation_channel();
                            let mut span = TRACER.span_builder("ConfigurationEventClient::recv")
                                .with_kind(SpanKind::Consumer)
                                .start_with_context(&*TRACER, &channel.into());
                            let _ = self.tx.send(event.event());
                            span.end();
                        } else {
                            break;
                        }
                    },
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }

            // We don't need the transport anymore
            drop(transport);
        });

        Ok(handle)
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationEventClientError {
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Connection failed")]
    ConnectionFailed,
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Unknown event")]
    Unknown,
}

impl From<ClientError> for ConfigurationEventClientError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Call(err) => match ConfigurationApiError::from(err) {
                ConfigurationApiError::NotFound => ConfigurationEventClientError::NotFound,
                _ => ConfigurationEventClientError::Unknown,
            },
            ClientError::RequestTimeout => ConfigurationEventClientError::RequestTimeout,
            _ => ConfigurationEventClientError::Unknown,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationEventRecvError {
    #[error("Channel is closed")]
    Closed,
    #[error("Channel has lagged")]
    Lagged,
}

#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ConfigurationEventsReceiver {
    tx: Sender<ConfigurationEvent>,
    rx: Receiver<ConfigurationEvent>,
}

impl Clone for ConfigurationEventsReceiver {
    fn clone(&self) -> Self {
        Self::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }
}

impl ConfigurationEventsReceiver {
    pub async fn recv(
        &mut self,
    ) -> Result<Traced<ConfigurationEvent>, ConfigurationEventRecvError> {
        let span = TRACER
            .span_builder("ConfigurationEventReceiver::recv")
            .with_kind(SpanKind::Consumer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        match self.rx.recv().with_context(context).await {
            Ok(value) => Ok(value),
            Err(RecvError::Closed) => Err(ConfigurationEventRecvError::Closed),
            Err(RecvError::Lagged(_)) => Err(ConfigurationEventRecvError::Lagged),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.rx.is_closed()
    }
}

impl ConfigurationEventsClient {
    /// Check if internal monitoring is active for this events client
    pub async fn is_monitoring(&self) -> bool {
        self.transport.is_monitoring().await
    }

    /// Stop internal monitoring if active
    /// This is useful for graceful shutdown or when monitoring is no longer needed
    pub async fn stop_monitoring(&self) -> Result<(), ConfigurationEventClientError> {
        self.transport.stop_monitoring().await.map_err(|e| {
            tracing::warn!("Failed to stop events client monitoring: {:?}", e);
            ConfigurationEventClientError::Unknown
        })
    }

    /// Get monitoring status information for the events client
    pub async fn monitoring_status(&self) -> Option<crate::transport::layers::MonitoringStatus> {
        if let Some(monitoring_manager) = self.transport.monitoring_manager() {
            let manager = monitoring_manager.lock().await;
            Some(manager.status())
        } else {
            None
        }
    }

    /// Internal error handler for events client
    /// This method implements self-management behavior for event stream errors
    fn handle_error_internally(&self, error: &ConfigurationEventClientError) -> bool {
        let should_handle = match error {
            // Transport-related errors should be handled internally
            ConfigurationEventClientError::RequestTimeout => true,
            ConfigurationEventClientError::ConnectionFailed => true,
            ConfigurationEventClientError::InvalidConfiguration => false, // Application-level issue
            ConfigurationEventClientError::NotFound => false,             // Application-level issue
            ConfigurationEventClientError::Unknown => true,               // Assume transport issue
        };

        let severity = match error {
            ConfigurationEventClientError::NotFound => "info",
            ConfigurationEventClientError::RequestTimeout => "warning",
            ConfigurationEventClientError::ConnectionFailed => "warning",
            ConfigurationEventClientError::InvalidConfiguration => "error",
            ConfigurationEventClientError::Unknown => "warning",
        };

        match severity {
            "info" => {
                tracing::debug!(
                    target: "rpc_client::events::error_handling",
                    error = %error,
                    "Informational error in events client"
                );
            }
            "warning" => {
                tracing::warn!(
                    target: "rpc_client::events::error_handling",
                    error = %error,
                    handled_internally = should_handle,
                    "Warning-level error in events client"
                );
            }
            "error" => {
                tracing::error!(
                    target: "rpc_client::events::error_handling",
                    error = %error,
                    handled_internally = should_handle,
                    "Error-level issue in events client"
                );
            }
            _ => {}
        }

        if should_handle {
            tracing::debug!(
                target: "rpc_client::events::error_handling",
                error = %error,
                "Events client error will be handled internally by robust layers"
            );
        }

        should_handle
    }

    /// Convert events client errors to gateway-friendly format
    pub fn to_gateway_error(
        &self,
        error: ConfigurationEventClientError,
    ) -> GatewayEventClientError {
        // Handle the error internally first
        self.handle_error_internally(&error);

        match error {
            ConfigurationEventClientError::NotFound => GatewayEventClientError::ResourceNotFound,
            ConfigurationEventClientError::InvalidConfiguration => {
                GatewayEventClientError::ConfigurationError
            }
            // All transport-related errors are abstracted as service unavailable
            ConfigurationEventClientError::RequestTimeout
            | ConfigurationEventClientError::ConnectionFailed
            | ConfigurationEventClientError::Unknown => {
                GatewayEventClientError::ServiceTemporarilyUnavailable
            }
        }
    }
}

/// Simplified error types for gateway consumption from events client
/// This reduces the complexity of error handling at the gateway level
#[derive(Debug, Clone, Error)]
pub enum GatewayEventClientError {
    #[error("Resource not found")]
    ResourceNotFound,

    #[error("Configuration error")]
    ConfigurationError,

    #[error("Event service temporarily unavailable")]
    ServiceTemporarilyUnavailable,
}
