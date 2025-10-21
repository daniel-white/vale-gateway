use std::ops::ControlFlow::Break;
use std::ops::Deref;
use std::time::Duration;
use jsonrpsee::core::client::SubscriptionClientT;
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
use vg_rpc::{ApiClient, ApiError, RequestContext, SubscribeEventsRequest};

use crate::events::error::RecvError;
use crate::instrumentation::TRACER;
use crate::transport::{Client, TransportClient};
pub use vg_rpc::Event;

pub struct EventClient {
    transport_client: TransportClient,
    tx: Sender<Event>,
}

impl EventClient {
    pub fn new(transport_client: TransportClient) -> Self {
        let (tx, _) = channel(32);
        Self { transport_client, tx }
    }

    pub fn events(&self) -> EventReceiver {
        EventReceiver::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut transport_client = self.transport_client;
        let tx = self.tx;
        
        spawn(async move {
            'main:
            loop {
                let Client::Connected(client) = transport_client.client() else {
                    select! {
                        result = transport_client.changed() => {
                            match result {
                                Ok(_) => continue 'main,
                                Err(_) => break 'main,
                            }
                        }
                        _ = stop_handle.stopped() => {
                            break 'main;
                        }
                    }
                };
                
                'subscription:
                loop {
                    let span = TRACER
                        .span_builder("EventClient::subscribe")
                        .with_kind(SpanKind::Client)
                        .start(&*TRACER);
                    let req = SubscribeEventsRequest::builder()
                        .context(RequestContext::new(span))
                        .listener_ref(transport_client.listener_ref())
                        .build();

                    select! {
                        result = transport_client.changed() => {
                            match result {
                                Ok(_) => continue 'main,
                                Err(_) => break 'main,
                            }
                        }
                        _ = stop_handle.stopped() => {
                            break 'main;
                        }
                        result = client.events(req) => {
                            let Ok(mut subscription) = result else {
                                // TODO add backoff and use tokio-retry
                                transport_client.request_reconnect().await;
                                tokio::time::sleep(Duration::from_secs(1)).await;
                                break 'subscription;
                            };

                            'recv:
                            loop {
                                select! {
                                    result = transport_client.changed() => {
                                        match result {
                                            Ok(_) => continue 'main,
                                            Err(_) => break 'main,
                                        }
                                    }
                                    _ = stop_handle.stopped() => {
                                        break 'main;
                                    }
                                    result = subscription.next() => {
                                        let Some(Ok(event)) = result else {
                                            continue 'subscription;
                                        };

                                        let channel = event.context().propagation_channel();
                                        let mut span = TRACER.span_builder("EventClient::recv")
                                            .with_kind(SpanKind::Consumer)
                                            .start_with_context(&*TRACER, &channel.into());
                                        let _ = tx.send(event.event());
                                        span.end();
                                    }
                                }
                            }
                        }
                    }
                
                }
        }
    });

    handle
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
    pub async fn recv(&mut self) -> Result<Traced<Event>, RecvError> {
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
