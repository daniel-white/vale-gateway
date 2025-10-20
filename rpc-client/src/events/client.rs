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

    /// Create a new ConfigurationEventsClient with basic connection (no robustness features)
    /// This maintains backward compatibility
    pub async fn connect(
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
