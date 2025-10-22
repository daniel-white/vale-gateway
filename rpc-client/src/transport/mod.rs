use getset::CloneGetters;
use http::Uri;
use jsonrpsee::ws_client::{PingConfig, WsClient, WsClientBuilder};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;
use vg_core::sync::handles::{Handle, handles};

#[derive(Debug, Clone)]
pub enum Client {
    Connected(Arc<WsClient>),
    Disconnected,
}

enum TransportClientMessage {
    Reconnect,
}

#[derive(Debug, Clone, CloneGetters, TypedBuilder)]
pub struct TransportClient {
    client: tokio::sync::watch::Receiver<Client>,
    #[getset(get_clone = "pub")]
    listener_ref: ListenerRef,
    message_tx: tokio::sync::mpsc::Sender<TransportClientMessage>,
}

impl TransportClient {
    pub async fn changed(&mut self) -> Result<(), ()> {
        self.client.changed().await.map_err(|_| ())
    }

    pub fn client(&self) -> Client {
        self.client.borrow().clone()
    }

    pub async fn request_reconnect(&self) {
        let _ = self
            .message_tx
            .send(TransportClientMessage::Reconnect)
            .await;
    }
}

#[derive(Debug, TypedBuilder)]
pub struct TransportOptions {
    #[builder(setter(into))]
    listener_ref: ListenerRef,
    #[builder(setter(into))]
    endpoint: Uri,
}

#[derive(TypedBuilder)]
pub struct Transport {
    listener_ref: ListenerRef,
    endpoint: Uri,
    client: tokio::sync::watch::Sender<Client>,
    message_rx: tokio::sync::mpsc::Receiver<TransportClientMessage>,
    message_tx: tokio::sync::mpsc::Sender<TransportClientMessage>,
}

impl Transport {
    pub fn client(&self) -> TransportClient {
        TransportClient::builder()
            .client(self.client.subscribe())
            .listener_ref(self.listener_ref.clone())
            .message_tx(self.message_tx.clone())
            .build()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            let mut message_rx = self.message_rx;
            loop {
                let _ = self.client.send(Client::Disconnected);
                let client = WsClientBuilder::new().enable_ws_ping(PingConfig::default());
                select! {
                    result = client.build(self.endpoint.to_string()) => {
                        match result {
                            Ok(client) => {
                                println!("client connected!");
                                let _ = self.client.send(Client::Connected(Arc::new(client)));
                                select! {
                                    _ = message_rx.recv() => {
                                        // TODO handle message error and types!
                                        let _ = self.client.send(Client::Disconnected);
                                        continue;
                                    }
                                    _ = stop_handle.stopped() => {
                                        let _ = self.client.send(Client::Disconnected);
                                        break;
                                    }
                                }
                            },
                            Err(_) => {
                                println!("client unable to connect!");

                                // TODO retry with staggered retries
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                continue;
                            }
                        }
                    }
                    _ = message_rx.recv() => {
                        // TODO handle message error and types!
                        continue;
                    }
                    _ = stop_handle.stopped() => {
                        let _ = self.client.send(Client::Disconnected);
                        break;
                    }
                }
            }
        });

        handle
    }
}

#[derive(Debug, Error)]
pub enum TransportConversionError {
    // TODO add errors for address
}

impl TryFrom<TransportOptions> for Transport {
    type Error = TransportConversionError;

    fn try_from(value: TransportOptions) -> Result<Self, Self::Error> {
        let (client, _) = tokio::sync::watch::channel(Client::Disconnected);
        let (message_tx, message_rx) = tokio::sync::mpsc::channel(10);

        let transport = Self::builder()
            .listener_ref(value.listener_ref)
            .endpoint(value.endpoint)
            .client(client)
            .message_tx(message_tx)
            .message_rx(message_rx)
            .build();

        Ok(transport)
    }
}
