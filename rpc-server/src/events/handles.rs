use tokio::sync::watch::{Receiver, Sender, channel};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ConfigurationEventPollingHandle {
    tx: Sender<()>,
}

#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("The server is already stopped")]
pub struct AlreadyStoppedError;

impl ConfigurationEventPollingHandle {
    pub async fn stopped(self) {
        self.tx.closed().await
    }
    pub fn shutdown(self) -> Result<(), AlreadyStoppedError> {
        self.tx.send(()).map_err(|_| AlreadyStoppedError)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationEventPollingStopHandle {
    rx: Receiver<()>,
}

impl ConfigurationEventPollingStopHandle {
    pub async fn stopped(&mut self) {
        let _ = self.rx.changed().await;
    }
}

pub fn polling_handles() -> (
    ConfigurationEventPollingHandle,
    ConfigurationEventPollingStopHandle,
) {
    let (tx, rx) = channel(());

    let polling_handle = ConfigurationEventPollingHandle::builder().tx(tx).build();

    let stop_handle = ConfigurationEventPollingStopHandle::builder()
        .rx(rx)
        .build();

    (polling_handle, stop_handle)
}
