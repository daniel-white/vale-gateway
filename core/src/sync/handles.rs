use tokio::sync::watch::{Receiver, Sender, channel};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct Handle {
    tx: Sender<()>,
}

#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("Already stopped")]
pub struct AlreadyStoppedError;

impl Handle {
    pub async fn stopped(self) {
        self.tx.closed().await
    }

    pub fn shutdown(self) -> Result<(), AlreadyStoppedError> {
        self.tx.send(()).map_err(|_| AlreadyStoppedError)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct StopHandle {
    rx: Receiver<()>,
}

impl StopHandle {
    pub async fn stopped(&mut self) {
        let _ = self.rx.changed().await;
    }
}

pub fn handles() -> (Handle, StopHandle) {
    let (tx, rx) = channel(());

    let handle = Handle::builder().tx(tx).build();

    let stop_handle = StopHandle::builder().rx(rx).build();

    (handle, stop_handle)
}
