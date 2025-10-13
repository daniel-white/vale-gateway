use thiserror::Error;
use tokio::sync::watch::{Receiver, Sender};
use typed_builder::TypedBuilder;

#[derive(Debug, Error)]
#[error("Error receiving")]
pub struct SourceConfigurationRecvError;

#[derive(Debug, Clone, TypedBuilder)]
pub struct SourceConfigurationWatch<T: Clone> {
    rx: Receiver<Option<T>>,
}

impl<T: Clone> SourceConfigurationWatch<T> {
    pub async fn changed(&mut self) -> Result<(), SourceConfigurationRecvError> {
        self.rx
            .changed()
            .await
            .map_err(|_| SourceConfigurationRecvError)
    }

    pub fn current(&self) -> Option<T> {
        self.rx.borrow().clone()
    }
}

#[derive(Debug, TypedBuilder)]
pub struct SourceConfigurationSender<T: Clone> {
    tx: Sender<Option<T>>,
}

#[derive(Debug, Error)]
#[error("Error sending")]
pub struct SourceConfigurationSendError;

impl<T: Clone> SourceConfigurationSender<T> {
    pub fn send(&self, val: T) -> Result<(), SourceConfigurationSendError> {
        self.tx
            .send(Some(val))
            .map_err(|_| SourceConfigurationSendError)
    }
}

pub fn channel<T: Clone>() -> (SourceConfigurationSender<T>, SourceConfigurationWatch<T>) {
    let (tx, rx) = tokio::sync::watch::channel(None);

    let sender = SourceConfigurationSender::builder().tx(tx).build();
    let watch = SourceConfigurationWatch::builder().rx(rx).build();

    (sender, watch)
}
