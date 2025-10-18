pub mod error;

use crate::instrumentation::TRACER;
use error::{RecvError, SendError};
use opentelemetry::Context;
use opentelemetry::trace::{FutureExt, SpanContext, SpanKind, TraceContextExt, Tracer};
use std::ops::Deref;

#[derive(Clone)]
pub struct WithContext<T: Clone> {
    value: T,
    span_context: SpanContext,
}

impl<T: Clone> From<T> for WithContext<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            span_context: Context::current().span().span_context().clone(),
        }
    }
}

pub struct Traced<T: Clone> {
    pub value: T,
    pub context: Context,
}

impl<T: Clone> From<WithContext<T>> for Traced<T> {
    fn from(value: WithContext<T>) -> Self {
        let context = Context::current().with_remote_span_context(value.span_context);
        Self {
            value: value.value,
            context,
        }
    }
}

impl<T: Clone> Deref for Traced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug)]
pub struct Receiver<T: Clone>(tokio::sync::broadcast::Receiver<WithContext<T>>);

impl<T: Clone> Receiver<T> {
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    pub async fn recv(&mut self) -> Result<Traced<T>, RecvError> {
        let span = TRACER
            .span_builder("broadcast::Sender::recv")
            .with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        self.0.recv().with_context(context).await.map(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct Sender<T: Clone>(tokio::sync::broadcast::Sender<WithContext<T>>);

impl<T: Clone> Sender<T> {
    pub fn send(&self, value: T) -> Result<usize, SendError<T>> {
        let span = TRACER
            .span_builder("broadcast::Sender::send")
            .with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let context = Context::current().with_span(span).attach();
        self.0
            .send(value.into())
            .map_err(|err| SendError(err.0.value))
    }

    pub fn subscribe(&self) -> Receiver<T> {
        Receiver(self.0.subscribe())
    }
}

pub fn channel<T: Clone>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);

    (Sender(tx), Receiver(rx))
}
