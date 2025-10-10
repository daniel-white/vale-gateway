use crate::ConfigurationClient;
use async_from::AsyncTryFrom;
use async_trait::async_trait;
use http::Uri;
use jsonrpsee::ws_client::WsClientBuilder;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast::channel;
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationClientOptions {
    #[builder(setter(into))]
    listener_ref: ListenerRef,
    #[builder(setter(into))]
    address: Uri,
}

#[derive(Debug, Error)]
pub enum ConfigurationClientInitError {
    #[error("WebSocket client error")]
    WsClientError,
}

#[async_trait]
impl AsyncTryFrom<ConfigurationClientOptions> for ConfigurationClient {
    type Error = ConfigurationClientInitError;

    async fn async_try_from(value: ConfigurationClientOptions) -> Result<Self, Self::Error> {
        let client = WsClientBuilder::new()
            .build(value.address.to_string())
            .await
            .map_err(|err| {
                println!("error creating ws client: {:?}", err);
                ConfigurationClientInitError::WsClientError
            })?;

        let (tx, _) = channel(10);
        let client = ConfigurationClient::builder()
            .listener_ref(value.listener_ref)
            .client(Arc::new(client))
            .tx(tx)
            .build();
        Ok(client)
    }
}
