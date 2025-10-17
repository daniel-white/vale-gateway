use crate::ConfigurationTransport;
use jsonrpsee::core::ClientError;
use opentelemetry::Context;
use opentelemetry::context::FutureExt;
use opentelemetry::trace::{Span, SpanKind, TraceContextExt, Tracer};
use thiserror::Error;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_core::sync::broadcast::error::RecvError;
use vg_core::sync::broadcast::{Receiver, Sender, Traced, WithContext, channel};
use vg_core::sync::handles::{Handle, handles};
use vg_rpc::{ConfigurationApiClient, ConfigurationApiError, RequestContext, SubscribeEventsRequest};

use crate::instrumentation::TRACER;
pub use vg_rpc::ConfigurationEvent;

pub struct ConfigurationEventClient {
    transport: ConfigurationTransport,
    tx: Sender<ConfigurationEvent>,
}

impl ConfigurationEventClient {
    pub fn new(transport: ConfigurationTransport) -> Self {
        let (tx, _) = channel(32);
        Self { transport, tx }
    }

    pub fn receiver(&self) -> ConfigurationEventReceiver {
        ConfigurationEventReceiver::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }

    pub async fn start(self) -> Result<Handle, ConfigurationEventClientError> {
        let span = TRACER
            .span_builder("ConfigurationEventClient::events")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let client = self.transport.client();
        let req = SubscribeEventsRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport.listener_ref())
            .build();

        let mut subscription = client.events(req).await?;
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
pub struct ConfigurationEventReceiver {
    tx: Sender<ConfigurationEvent>,
    rx: Receiver<ConfigurationEvent>,
}

impl Clone for ConfigurationEventReceiver {
    fn clone(&self) -> Self {
        Self::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }
}

impl ConfigurationEventReceiver {
    pub async fn recv(
        &mut self,
    ) -> Result<Traced<ConfigurationEvent>, ConfigurationEventRecvError> {
        let span = TRACER.span_builder("ConfigurationEventReceiver::recv")
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
