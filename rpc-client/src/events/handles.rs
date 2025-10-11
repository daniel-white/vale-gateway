use thiserror::Error;
use tokio::sync::watch::{Receiver, Sender, channel};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ConfigurationEventClientHandle {
    tx: Sender<()>,
}

#[derive(Debug, Copy, Clone, Error)]
#[error("The client is already stopped")]
pub struct AlreadyStoppedError;

impl ConfigurationEventClientHandle {
    pub async fn stopped(self) {
        self.tx.closed().await
    }
    pub fn shutdown(self) -> Result<(), AlreadyStoppedError> {
        self.tx.send(()).map_err(|_| AlreadyStoppedError)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationEventClientStopHandle {
    rx: Receiver<()>,
}

impl ConfigurationEventClientStopHandle {
    pub async fn stopped(&mut self) {
        let _ = self.rx.changed().await;
    }
}

pub(crate) fn handles() -> (
    ConfigurationEventClientHandle,
    ConfigurationEventClientStopHandle,
) {
    let (tx, rx) = channel(());

    let server_handle = ConfigurationEventClientHandle::builder().tx(tx).build();

    let stop_handle = ConfigurationEventClientStopHandle::builder().rx(rx).build();

    (server_handle, stop_handle)
}
