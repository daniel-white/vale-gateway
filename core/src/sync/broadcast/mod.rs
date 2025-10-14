pub mod error;

use std::ops::Deref;
use opentelemetry::{Context, ContextGuard};
use error::{SendError, RecvError};

#[derive(Clone)]
struct Message<T: Clone> {
    value: T,
    context: Context
}

impl <T: Clone> From<T> for Message<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            context: Context::current()
        }
    }
}

impl <T: Clone> From<Message<T>> for TracedValue<T> {
    fn from(value: Message<T>) -> Self {
        Self {
            value: value.value,
            guard: value.context.attach(),
        }
    }
}

pub struct TracedValue<T : Clone> {
    value: T,
    guard: ContextGuard
}

unsafe impl <T: Clone> Send for TracedValue<T> {}

impl <T: Clone> Deref for TracedValue<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug)]
pub struct Receiver<T: Clone>(tokio::sync::broadcast::Receiver<Message<T>>);

impl <T: Clone> Receiver<T> {
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
    
    pub async fn recv(&mut self) -> Result<TracedValue<T>, RecvError> {
        self.0.recv().await.map(Into::into)
    }
}

#[derive(Debug, Clone)]
pub struct Sender<T: Clone>(tokio::sync::broadcast::Sender<Message<T>>);

impl<T: Clone> Sender<T> {
    pub fn send(&self, value: T) -> Result<usize, SendError<T>> {
        self.0.send(value.into()).map_err(|err| SendError(err.0.value))
    }

    pub fn subscribe(&self) -> Receiver<T> {
        Receiver(self.0.subscribe())
    }
}

pub fn channel<T: Clone>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);

    (Sender(tx), Receiver(rx))
}