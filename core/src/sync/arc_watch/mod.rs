pub  mod error;

use std::ops::Deref;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use error::{RecvError, SendError};


#[derive(Debug, Clone, TypedBuilder)]
pub struct Receiver<T> {
    rx: tokio::sync::watch::Receiver<Option<Arc<T>>>,
}

impl<T> Receiver<T> {
    pub async fn changed(&mut self) -> Result<(), RecvError> {
        self.rx.changed().await
    }

    pub fn current(&self) -> Option<Arc<T>> {
        self.rx.borrow().clone()
    }
}

#[derive(Debug, TypedBuilder)]
pub struct Sender<T> {
    tx: tokio::sync::watch::Sender<Option<Arc<T>>>,
}

impl<T> Sender<T> {
    pub fn send(&self, val: Arc<T>) -> Result<(), SendError<Arc<T>>> {
        self.tx.send(Some(val.clone()))
            .map_err(|_| SendError(val))
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::watch::channel(None);

    let sender = Sender::builder().tx(tx).build();
    let watch = Receiver::builder().rx(rx).build();

    (sender, watch)
}
