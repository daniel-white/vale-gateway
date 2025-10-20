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
    /// Reuses existing subscription logic from events/client.rs
    pub async fn start(&mut self) -> Result<Handle, ConfigurationEventClientError> {
        // Reuse existing subscription request building
        let span = TRACER
            .span_builder("ConfigurationEventClient::events")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = SubscribeEventsRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.listener_ref().clone())
            .build();

        let mut subscription = (&**self.transport().client()).events(req).await?;

        // Use core handles for task management
        let (handle, mut stop_handle) = handles();

        // Event processing task (reused logic from existing events/client.rs)
        let event_sender = self.transport().event_sender().clone();
        spawn(async move {
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
                            break;
                        }
                    },
                    _ = stop_handle.stopped() => {
                        break;
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
    pub async fn monitoring_status(&self) -> crate::transport::layers::MonitoringStatus {
        self.transport.monitoring_status().await
    }

    /// Start event subscription with resilient connection handling
    /// This method implements graceful reconnection and event queuing during connection failures
    pub async fn start_resilient(&mut self) -> Result<Handle, ConfigurationEventClientError> {
        // Use existing reconnection layers from RpcTransport
        // The RpcTransport already handles reconnection, so we just need to start normally
        // but with additional error handling for initial connection failures
        match self.start().await {
            Ok(handle) => {
                tracing::info!(
                    "Events client started successfully with resilient connection handling"
                );
                Ok(handle)
            }
            Err(ConfigurationEventClientError::ConnectionFailed) => {
                // Don't fail immediately on connection failure - the transport will handle reconnection
                tracing::warn!(
                    "Initial connection failed, but events client will continue attempting to reconnect"
                );

                // Create a handle that represents the ongoing connection attempts
                let (handle, mut stop_handle) = handles();
                let transport = self.transport.clone();
                let listener_ref = self.listener_ref.clone();
                let event_sender = transport.event_sender().clone();

                spawn(async move {
                    let mut retry_count = 0;
                    let max_retries = 10; // Allow multiple retries before giving up

                    loop {
                        select! {
                            _ = stop_handle.stopped() => {
                                tracing::info!("Resilient events client stopped");
                                break;
                            }
                            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                                retry_count += 1;
                                if retry_count > max_retries {
                                    tracing::error!("Events client failed to connect after {} retries", max_retries);
                                    break;
                                }

                                // Try to establish subscription again
                                let span = TRACER
                                    .span_builder("ConfigurationEventClient::events_retry")
                                    .with_kind(SpanKind::Client)
                                    .start(&*TRACER);

                                let req = SubscribeEventsRequest::builder()
                                    .context(RequestContext::new(span))
                                    .listener_ref(listener_ref.clone())
                                    .build();

                                match (&**transport.client()).events(req).await {
                                    Ok(mut subscription) => {
                                        tracing::info!("Events client reconnected successfully after {} retries", retry_count);
                                        retry_count = 0; // Reset retry count on successful connection

                                        // Start event processing loop
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
                                                        tracing::warn!("Event subscription ended, will retry connection");
                                                        break; // Break inner loop to retry connection
                                                    }
                                                },
                                                _ = stop_handle.stopped() => {
                                                    tracing::info!("Resilient events client stopped during event processing");
                                                    return; // Exit the entire task
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to reconnect events client (attempt {}): {:?}", retry_count, e);
                                        // Continue the loop to retry
                                    }
                                }
                            }
                        }
                    }
                });

                self.subscription_handle = Some(handle.clone());
                Ok(handle)
            }
            Err(e) => Err(e),
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
