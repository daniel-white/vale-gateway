use std::sync::Arc;
use thiserror::Error;
use tokio::sync::watch::{Receiver, Sender, channel as watch_channel};
use typed_builder::TypedBuilder;

#[derive(Debug, Error)]
#[error("Error receiving")]
pub struct ConfigurationRecvError;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ConfigurationWatch<T> {
    rx: Receiver<Option<Arc<T>>>,
}

impl<T> ConfigurationWatch<T> {
    pub async fn changed(&mut self) -> Result<(), ConfigurationRecvError> {
        self.rx
            .changed()
            .await
            .map_err(|_| ConfigurationRecvError)
    }

    pub fn current(&self) -> Option<Arc<T>> {
        self.rx.borrow().clone()
    }
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationSender<T> {
    tx: Sender<Option<Arc<T>>>,
}

#[derive(Debug, Error)]
#[error("Error sending")]
pub struct ConfigurationSendError;

impl<T> ConfigurationSender<T> {
    pub fn send(&self, val: Arc<T>) -> Result<(), ConfigurationSendError> {
        self.tx
            .send(Some(val))
            .map_err(|_| ConfigurationSendError)
    }
}

pub fn channel<T>() -> (ConfigurationSender<T>, ConfigurationWatch<T>) {
    let (tx, rx) = watch_channel(None);

    let sender = ConfigurationSender::builder().tx(tx).build();
    let watch = ConfigurationWatch::builder().rx(rx).build();

    (sender, watch)
}
