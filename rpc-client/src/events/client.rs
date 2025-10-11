use crate::ConfigurationTransport;
use vg_core::sync::handles::{handles, Handle};
use jsonrpsee::core::ClientError;
use thiserror::Error;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender, channel};
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_rpc::{ConfigurationApiClient, ConfigurationApiError, ConfigurationEvent};

#[derive(Debug)]
pub struct ConfigurationEventClient {
    transport: ConfigurationTransport,
    tx: Sender<ConfigurationEvent>,
}

impl ConfigurationEventClient {
    pub fn new(transport: ConfigurationTransport) -> Self {
        let (tx, _) = channel(32);
        Self { transport, tx }
    }

    pub fn receiver(&self) -> ConfigurationEventReceiver {
        ConfigurationEventReceiver::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }

    pub async fn start(
        self,
    ) -> Result<Handle, ConfigurationEventClientError> {
        let client = self.transport.client();
        let listener_ref = self.transport.listener_ref();
        let subscription = client.events(listener_ref).await?;
        let (handle, stop_handle) = handles();

        spawn(async move {
            let tx = self.tx;
            let mut subscription = subscription;
            let mut stop_handle = stop_handle;
            loop {
                select! {
                    event = subscription.next() => {
                        if let Some(Ok(event)) = event {
                            let _ = tx.send(event);
                        }
                    },
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationEventClientError {
    #[error("Listener not found")]
    NotFound(#[source] ClientError),
    #[error("Request timeout")]
    RequestTimeout(#[source] ClientError),
    #[error("Unknown")]
    Unknown,
}

impl From<ClientError> for ConfigurationEventClientError {
    fn from(value: ClientError) -> Self {
        match &value {
            ClientError::Call(err) => match ConfigurationApiError::from(err) {
                ConfigurationApiError::NotFound => ConfigurationEventClientError::NotFound(value),
                _ => ConfigurationEventClientError::Unknown,
            },
            ClientError::RequestTimeout => ConfigurationEventClientError::RequestTimeout(value),
            _ => ConfigurationEventClientError::Unknown,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationEventRecvError {
    #[error("Channel is closed")]
    Closed,
    #[error("Channel has lagged")]
    Lagged,
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationEventReceiver {
    tx: Sender<ConfigurationEvent>,
    rx: Receiver<ConfigurationEvent>,
}

impl Clone for ConfigurationEventReceiver {
    fn clone(&self) -> Self {
        Self::builder()
            .tx(self.tx.clone())
            .rx(self.tx.subscribe())
            .build()
    }
}

impl ConfigurationEventReceiver {
    pub async fn recv(&mut self) -> Result<ConfigurationEvent, ConfigurationEventRecvError> {
        match self.rx.recv().await {
            Ok(event) => Ok(event),
            Err(RecvError::Closed) => Err(ConfigurationEventRecvError::Closed),
            Err(RecvError::Lagged(_)) => Err(ConfigurationEventRecvError::Lagged),
        }
    }
    
    pub fn is_closed(&self) -> bool {
        self.rx.is_closed()
    }
}
