use crate::transport::rpc::RpcTransport;
use getset::Getters;
use jsonrpsee::core::ClientError;
use opentelemetry::Context;
use opentelemetry::context::FutureExt;
use opentelemetry::trace::{Span, SpanKind, TraceContextExt, Tracer};
use thiserror::Error;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_core::sync::broadcast::error::RecvError;
use vg_core::sync::broadcast::{Receiver, Sender, Traced};
use vg_core::sync::handles::{Handle, handles};
use vg_rpc::{
    ConfigurationApiClient, ConfigurationApiError, RequestContext, SubscribeEventsRequest,
};

use crate::instrumentation::TRACER;
pub use vg_rpc::ConfigurationEvent;

/// Configuration events client that uses RPC transport
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Getters, TypedBuilder)]
pub struct ConfigurationEventsClient {
    /// RPC transport instance
    #[getset(get = "pub")]
    transport: RpcTransport,

    /// Listener reference for this client
    #[getset(get = "pub")]
    listener_ref: ListenerRef,

    /// Event receiver using core broadcast channels
    #[getset(get = "pub")]
    event_receiver: Receiver<ConfigurationEvent>,

    /// Subscription handle using core handles
    #[getset(get = "pub")]
    #[builder(default)]
    subscription_handle: Option<Handle>,
}

impl ConfigurationEventsClient {
    /// Create a new ConfigurationEventsClient with RPC transport
    /// This replaces the connect() method - transport is now passed in
    pub fn new(transport: RpcTransport, listener_ref: impl Into<ListenerRef>) -> Self {
        let listener_ref = listener_ref.into();
        let event_receiver = transport.event_sender().subscribe();

        Self::builder()
            .transport(transport)
            .listener_ref(listener_ref)
            .event_receiver(event_receiver)
            .build()
    }

    pub fn events(&self) -> ConfigurationEventsReceiver {
        ConfigurationEventsReceiver::builder()
            .tx(self.transport.event_sender().clone())
            .rx(self.transport.event_sender().subscribe())
            .build()
    }

    /// Start event subscription on RPC transport connection
    /// Relies entirely on the transport for connection management - no retry logic here
    pub async fn start(&mut self) -> Result<Handle, ConfigurationEventClientError> {
        // Use core handles for task management
        let (handle, mut stop_handle) = handles();

        // Clone necessary data for the persistent task
        let transport = self.transport().clone();
        let listener_ref = self.listener_ref().clone();
        let event_sender = self.transport().event_sender().clone();
        let mut connection_state_receiver = self.transport().connection_state_sender().subscribe();

        // Persistent event processing task
        spawn(async move {
            tracing::info!(
                "ConfigurationEventsClient started - relying on transport for connection management"
            );

            loop {
                // Attempt to establish subscription using transport's current connection
                let span = TRACER
                    .span_builder("ConfigurationEventClient::events")
                    .with_kind(SpanKind::Client)
                    .start(&*TRACER);

                let req = SubscribeEventsRequest::builder()
                    .context(RequestContext::new(span))
                    .listener_ref(listener_ref.clone())
                    .build();

                // Use current_client() to get the transport's current connection
                let current_client = transport.current_client().await;
                match (**current_client).events(req).await {
                    Ok(mut subscription) => {
                        tracing::debug!(
                            "ConfigurationEventsClient subscription established successfully"
                        );

                        // Process events until connection fails or shutdown
                        loop {
                            select! {
                                event = subscription.next() => {
                                    if let Some(Ok(event)) = event {
                                        let channel = event.context().propagation_channel();
                                        let mut span = TRACER.span_builder("ConfigurationEventClient::recv")
                                            .with_kind(SpanKind::Consumer)
                                            .start_with_context(&*TRACER, &channel.into());
                                        let _ = event_sender.send(event.event());
                                        span.end();
                                    } else {
                                        // Subscription ended - let transport handle reconnection
                                        tracing::debug!("ConfigurationEventsClient subscription ended, waiting for transport to reconnect");
                                        break; // Break inner loop to wait for transport reconnection
                                    }
                                },
                                connection_state = connection_state_receiver.recv() => {
                                    if let Ok(traced_state) = connection_state {
                                        match traced_state.value {
                                            crate::transport::rpc::ConnectionState::Connected => {
                                                tracing::debug!("ConfigurationEventsClient detected transport reconnected, will reestablish subscription");
                                                break; // Break inner loop to immediately reestablish subscription
                                            },
                                            crate::transport::rpc::ConnectionState::Disconnected => {
                                                tracing::debug!("ConfigurationEventsClient detected transport disconnected");
                                                // Continue processing current subscription until it fails naturally
                                            },
                                            crate::transport::rpc::ConnectionState::Reconnecting => {
                                                tracing::debug!("ConfigurationEventsClient detected transport reconnecting");
                                                // Continue processing current subscription until it fails naturally
                                            }
                                        }
                                    }
                                },
                                _ = stop_handle.stopped() => {
                                    tracing::info!("ConfigurationEventsClient stopped during event processing");
                                    return; // Exit the entire task
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Subscription failed - trigger transport reconnection and wait for it to complete
                        tracing::debug!(
                            "ConfigurationEventsClient subscription failed: {:?}, triggering transport reconnection",
                            e
                        );

                        // Trigger reconnection on the transport
                        if let Err(reconnect_err) = transport.trigger_reconnection().await {
                            tracing::warn!(
                                "Failed to trigger transport reconnection: {:?}",
                                reconnect_err
                            );
                        }

                        // Wait for transport to signal it's connected before retrying
                        loop {
                            select! {
                                connection_state = connection_state_receiver.recv() => {
                                    if let Ok(traced_state) = connection_state {
                                        match traced_state.value {
                                            crate::transport::rpc::ConnectionState::Connected => {
                                                tracing::debug!("ConfigurationEventsClient detected transport reconnected, will retry subscription");
                                                break; // Break out of wait loop to retry subscription
                                            },
                                            crate::transport::rpc::ConnectionState::Disconnected => {
                                                tracing::debug!("ConfigurationEventsClient detected transport disconnected, continuing to wait");
                                            },
                                            crate::transport::rpc::ConnectionState::Reconnecting => {
                                                tracing::debug!("ConfigurationEventsClient detected transport reconnecting, continuing to wait");
                                            }
                                        }
                                    }
                                },
                                _ = stop_handle.stopped() => {
                                    tracing::info!("ConfigurationEventsClient stopped while waiting for transport reconnection");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        });

        self.subscription_handle = Some(handle.clone());
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
    #[error("Events not supported by server")]
    EventsNotSupported,
    #[error("Unknown event")]
    Unknown,
}

impl ConfigurationEventClientError {
    /// Determine if tasks should complete based on this error
    /// Uses existing error classification to determine task continuation
    /// Only complete tasks for truly unrecoverable errors (shutdown signals)
    pub fn should_complete_task(&self) -> bool {
        match self {
            // Temporary errors - tasks should continue
            ConfigurationEventClientError::RequestTimeout => false,
            ConfigurationEventClientError::ConnectionFailed => false,
            ConfigurationEventClientError::Unknown => false,

            // Application-level errors - tasks should complete
            ConfigurationEventClientError::NotFound => true,
            ConfigurationEventClientError::InvalidConfiguration => true,
            ConfigurationEventClientError::EventsNotSupported => true,
        }
    }

    /// Returns true if the error indicates a temporary failure
    pub fn is_temporary(&self) -> bool {
        match self {
            ConfigurationEventClientError::RequestTimeout => true,
            ConfigurationEventClientError::ConnectionFailed => true,
            ConfigurationEventClientError::Unknown => true,
            ConfigurationEventClientError::NotFound => false,
            ConfigurationEventClientError::InvalidConfiguration => false,
            ConfigurationEventClientError::EventsNotSupported => false,
        }
    }
}

impl From<ClientError> for ConfigurationEventClientError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Call(err) => match ConfigurationApiError::from(err) {
                ConfigurationApiError::NotFound => ConfigurationEventClientError::NotFound,
                _ => ConfigurationEventClientError::Unknown,
            },
            ClientError::RequestTimeout => ConfigurationEventClientError::RequestTimeout,
            ClientError::Transport(_) => ConfigurationEventClientError::ConnectionFailed,
            ClientError::RestartNeeded(_) => ConfigurationEventClientError::ConnectionFailed,
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
    pub async fn monitoring_status(&self) -> crate::transport::layers::MonitoringStatus {
        self.transport.monitoring_status().await
    }

    /// Start event subscription with resilient connection handling
    /// This method is now identical to start() since all resilience is handled by the transport
    pub async fn start_resilient(&mut self) -> Result<Handle, ConfigurationEventClientError> {
        tracing::info!(
            "Starting resilient events client - all connection management delegated to transport"
        );
        self.start().await
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
            ConfigurationEventClientError::EventsNotSupported => false,   // Application-level issue
        };

        let severity = match error {
            ConfigurationEventClientError::NotFound => "info",
            ConfigurationEventClientError::RequestTimeout => "warning",
            ConfigurationEventClientError::ConnectionFailed => "warning",
            ConfigurationEventClientError::InvalidConfiguration => "error",
            ConfigurationEventClientError::Unknown => "warning",
            ConfigurationEventClientError::EventsNotSupported => "error",
        };

        match severity {
            "info" => {
                tracing::debug!(
                    target: "vg_rpc_client::events::error_handling",
                    error = %error,
                    "Informational error in events client"
                );
            }
            "warning" => {
                tracing::warn!(
                    target: "vg_rpc_client::events::error_handling",
                    error = %error,
                    handled_internally = should_handle,
                    "Warning-level error in events client"
                );
            }
            "error" => {
                tracing::error!(
                    target: "vg_rpc_client::events::error_handling",
                    error = %error,
                    handled_internally = should_handle,
                    "Error-level issue in events client"
                );
            }
            _ => {}
        }

        if should_handle {
            tracing::debug!(
                target: "vg_rpc_client::events::error_handling",
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
            ConfigurationEventClientError::EventsNotSupported => {
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
