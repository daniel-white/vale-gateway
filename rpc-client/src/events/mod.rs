pub mod error;

use std::time::Duration;
use typed_builder::TypedBuilder;
use tokio::{select, spawn};
use opentelemetry::trace::{Span, SpanKind, TraceContextExt, Tracer};
use thiserror::Error;
use opentelemetry::Context;
use opentelemetry::context::FutureExt;
use vg_core::sync::broadcast::{Receiver, Sender, Traced};
use vg_core::sync::handles::{handles, Handle};
use vg_rpc::{ApiClient, RequestContext, SubscribeEventsRequest};
use crate::events::error::RecvError;
use crate::instrumentation::TRACER;
use crate::transport::{Client, TransportClient};

pub use vg_rpc::Event;

#[derive(TypedBuilder)]
pub struct EventClientOptions {
    transport_client: TransportClient,
    capacity: usize,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct EventClient {
    transport_client: TransportClient,
    sender: Sender<Event>,
}

impl EventClient {
    pub fn events(&self) -> EventReceiver {
        EventReceiver::builder()
            .tx(self.sender.clone())
            .rx(self.sender.subscribe())
            .build()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut transport_client = self.transport_client;
        let sender = self.sender;

        spawn(async move {
            'main: loop {
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

                'subscription: loop {
                    let span = TRACER
                        .span_builder("EventClient::subscribe")
                        .with_kind(SpanKind::Client)
                        .start(&*TRACER);
                    let req = SubscribeEventsRequest::builder()
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
                            println!("subscription result: {:?}", result);
                            let Ok(mut subscription) = result else {
                                // TODO add backoff and use tokio-retry
                                println!("subscription failed");
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
                                            continue 'main;
                                        };

                                        let channel = event.context().propagation_channel();
                                        let mut span = TRACER.span_builder("EventClient::recv")
                                            .with_kind(SpanKind::Consumer)
                                            .start_with_context(&*TRACER, &channel.into());
                                        let _ = sender.send(event.event());
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
pub enum EventClientConversionError {}

impl TryFrom<EventClientOptions> for EventClient {
    type Error = EventClientConversionError;

    fn try_from(value: EventClientOptions) -> Result<Self, Self::Error> {
        let (sender, _) = vg_core::sync::broadcast::channel(value.capacity);

        let client = Self::builder()
            .transport_client(value.transport_client)
            .sender(sender)
            .build();

        Ok(client)
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
            Err(tokio::sync::broadcast::error::RecvError::Closed) => Err(RecvError::Closed),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => Err(RecvError::Lagged),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.rx.is_closed()
    }
}