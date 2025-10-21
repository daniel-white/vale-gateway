use jsonrpsee::core::ClientError;
use opentelemetry::Context;
use opentelemetry::context::FutureExt;
use opentelemetry::trace::{Span, SpanKind, TraceContextExt, Tracer};
use thiserror::Error;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_core::sync::broadcast::error::RecvError as BroadcastRecvErr;
use vg_core::sync::broadcast::{Receiver, Sender, Traced, channel};
use vg_core::sync::handles::{Handle, handles};
use vg_rpc::{
    ApiClient, ApiError, RequestContext, SubscribeEventsRequest,
};

use crate::instrumentation::TRACER;
pub use vg_rpc::Event;
use crate::events::error::RecvError;
use crate::transport::Transport;

pub struct EventClient {
    transport: Transport,
    tx: Sender<Event>,
}

impl EventClient {
    pub fn new(transport: Transport) -> Self {
        let (tx, _) = channel(32);
        Self { transport, tx }
    }

    pub fn events(&self) -> EventReceiver {
        EventReceiver::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }

    pub async fn start(self) -> Result<Handle, EventClientError> {
        let span = TRACER
            .span_builder("EventClient::start")
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
                            let mut span = TRACER.span_builder("EventClient::recv")
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
pub enum EventClientError {
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Unknown event")]
    Unknown,
}

impl From<ClientError> for EventClientError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Call(err) => match ApiError::from(err) {
                ApiError::NotFound => EventClientError::NotFound,
                _ => EventClientError::Unknown,
            },
            ClientError::RequestTimeout => EventClientError::RequestTimeout,
            _ => EventClientError::Unknown,
        }
    }
}



#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct EventReceiver {
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Clone for EventReceiver {
    fn clone(&self) -> Self {
        Self::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }
}

impl EventReceiver {
    pub async fn recv(
        &mut self,
    ) -> Result<Traced<Event>, RecvError> {
        let span = TRACER
            .span_builder("EventReceiver::recv")
            .with_kind(SpanKind::Consumer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        match self.rx.recv().with_context(context).await {
            Ok(value) => Ok(value),
            Err(BroadcastRecvErr::Closed) => Err(RecvError::Closed),
            Err(BroadcastRecvErr::Lagged(_)) => Err(RecvError::Lagged),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.rx.is_closed()
    }
}
