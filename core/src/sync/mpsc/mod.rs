pub mod error;

use std::ops::{Deref};
use opentelemetry::{Context, ContextGuard};
use opentelemetry::trace::FutureExt;
use error::SendError;

struct Message<T> {
    value: T,
    context: Context
}

impl <T> From<T> for Message<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            context: Context::current()
        }
    }
}

impl <T> From<Message<T>> for TracedValue<T> {
    fn from(value: Message<T>) -> Self {
        Self {
            value: value.value,
            guard: value.context.attach(),
        }
    }
}

pub struct TracedValue<T> {
    value: T,
    guard: ContextGuard
}

unsafe impl <T> Send for TracedValue<T>{}

impl <T> Deref for TracedValue<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug)]
pub struct Receiver<T>(tokio::sync::mpsc::Receiver<Message<T>>);





impl <T> Receiver<T> {
    pub async fn recv(&mut self) -> Option<TracedValue<T>> {
           self.0.recv().with_current_context().await.map(Into::into)
    }
}

#[derive(Clone, Debug)]
pub struct Sender<T>(tokio::sync::mpsc::Sender<Message<T>>);

impl <T> Sender<T> {
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
         self.0.send(value.into()).with_current_context().await.map_err(|err| SendError(err.0.value))
    }
}

pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::mpsc::channel(buffer);
    (Sender(tx), Receiver(rx))
}
