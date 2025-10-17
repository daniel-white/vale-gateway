pub mod error;

use std::ops::Deref;
use error::SendError;
use opentelemetry::{Context};
use opentelemetry::context::FutureExt;
use opentelemetry::trace::{SpanContext, SpanKind, TraceContextExt, Tracer};
use crate::instrumentation::TRACER;

pub struct WithContext<T> {
    value: T,
    span_context: SpanContext
}
impl<T> From<T> for WithContext<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            span_context: Context::current().span().span_context().clone(),
        }
    }
}

pub struct Traced<T> {
    pub value: T,
    pub context: Context,
}

impl <T> From<WithContext<T>> for Traced<T> {
    fn from(value: WithContext<T>) -> Self {
        let context = Context::current().with_remote_span_context(value.span_context);
        Self {
            value: value.value,
            context
        }
    }
}

impl <T> Deref for  Traced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}


#[derive(Debug)]
pub struct Receiver<T>(tokio::sync::mpsc::Receiver<WithContext<T>>);

impl<T> Receiver<T> {
    pub async fn recv(&mut self) -> Option<Traced<T>> {
        let span = TRACER.span_builder(
            "mpsc::Receiver::recv"
        ).with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        self.0.recv().with_context(context).await.map(Into::into)
    }
}

#[derive(Clone, Debug)]
pub struct Sender<T>(tokio::sync::mpsc::Sender<WithContext<T>>);

impl<T> Sender<T> {
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        let span = TRACER.span_builder(
            "mpsc::Sender::send"
        ).with_kind(SpanKind::Producer)
            .start(&*TRACER);
        let context = Context::current().with_span(span);
        self.0
            .send(value.into())
            .with_context(context)
            .await
            .map_err(|err| SendError(err.0.value))
    }
}

pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::mpsc::channel(buffer);
    (Sender(tx), Receiver(rx))
}
