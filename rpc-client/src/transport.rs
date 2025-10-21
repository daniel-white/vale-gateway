use async_from::AsyncTryFrom;
use async_trait::async_trait;
use getset::CloneGetters;
use http::Uri;
use jsonrpsee::ws_client::{PingConfig, WsClient, WsClientBuilder};
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;

#[derive(Clone, CloneGetters, TypedBuilder)]
pub struct Transport {
    #[getset(get_clone = "pub(crate)")]
    listener_ref: ListenerRef,
    #[getset(get_clone = "pub(crate)")]
    client: Arc<WsClient>,
}

#[derive(Debug, TypedBuilder)]
pub struct TransportOptions {
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
impl AsyncTryFrom<TransportOptions> for Transport {
    type Error = ConfigurationClientInitError;

    async fn async_try_from(value: TransportOptions) -> Result<Self, Self::Error> {
        let client = WsClientBuilder::new()
            .enable_ws_ping(PingConfig::default())
            .build(value.address.to_string())
            .await
            .map_err(|err| {
                println!("error creating ws client: {:?}", err);
                ConfigurationClientInitError::WsClientError
            })?;

        let transport = Transport::builder()
            .listener_ref(value.listener_ref)
            .client(Arc::new(client))
            .build();

        Ok(transport)
    }
}
