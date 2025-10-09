use jsonrpsee::core::ClientError;
use jsonrpsee::ws_client::WsClient;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast::{Receiver, Sender};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::{Listener, ListenerRef};
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{ConfigurationApiClient, ConfigurationApiError};

pub use vg_rpc::ConfigurationEvent;

#[derive(Debug)]
pub enum ConfigurationClientError {
    NotFound,
    RequestTimeout,
    Unknown,
}

impl From<ClientError> for ConfigurationClientError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Call(err) => match ConfigurationApiError::from(err) {
                ConfigurationApiError::NotFound => ConfigurationClientError::NotFound,
                _ => ConfigurationClientError::Unknown,
            },
            ClientError::RequestTimeout => ConfigurationClientError::RequestTimeout,
            _ => ConfigurationClientError::Unknown,
        }
    }
}

impl From<ConfigurationApiError> for ConfigurationClientError {
    fn from(value: ConfigurationApiError) -> Self {
        match value {
            ConfigurationApiError::NotFound => ConfigurationClientError::NotFound,
            _ => ConfigurationClientError::Unknown,
        }
    }
}

#[derive(Debug, Clone, TypedBuilder)]
pub struct ConfigurationClient {
    listener_ref: ListenerRef,
    client: Arc<WsClient>,
    tx: Sender<ConfigurationEvent>,
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Listener, ConfigurationClientError> {
        Ok(self.client.listener(self.listener_ref.clone()).await?)
    }

    pub async fn route(&self, route_ref: &RouteRef) -> Result<Route, ConfigurationClientError> {
        Ok(self.client.route(route_ref.clone()).await?)
    }

    pub async fn backend(
        &self,
        backend_ref: &BackendRef,
    ) -> Result<Backend, ConfigurationClientError> {
        Ok(self.client.backend(backend_ref.clone()).await?)
    }

    pub async fn watch_events(&self) -> Result<(), ConfigurationClientError> {
        let mut subscription = self.client.watch_events(self.listener_ref.clone()).await?;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            while let Some(event) = subscription.next().await {
                match event {
                    Ok(ev) => {
                        let _ = tx.send(ev);
                    }
                    Err(_) => {
                        // Handle error (e.g., log it)
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    pub fn event_receiver(&self) -> ConfigurationEventReceiver {
        let rx = self.tx.subscribe();
        ConfigurationEventReceiver::builder().rx(rx).build()
    }
}

#[derive(Debug, Error)]
#[error("Failed to receive configuration event")]
pub struct ConfigurationEventRecvError;

#[derive(TypedBuilder)]
pub struct ConfigurationEventReceiver {
    rx: Receiver<ConfigurationEvent>,
}

impl ConfigurationEventReceiver {
    pub async fn recv(&mut self) -> Result<ConfigurationEvent, ConfigurationEventRecvError> {
        match self.rx.recv().await {
            Ok(event) => Ok(event),
            Err(_) => Err(ConfigurationEventRecvError),
        }
    }
}
