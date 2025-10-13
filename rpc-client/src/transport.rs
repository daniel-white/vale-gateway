use crate::middleware::tracing::{Tracing, TracingLayer};
use async_from::AsyncTryFrom;
use async_trait::async_trait;
use getset::CloneGetters;
use http::Uri;
use jsonrpsee::core::middleware::layer::{RpcLogger, RpcLoggerLayer};
use jsonrpsee::ws_client::{RpcService, RpcServiceBuilder, WsClient, WsClientBuilder};
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::listener::ListenerRef;

#[derive(Clone, CloneGetters, TypedBuilder)]
pub struct ConfigurationTransport {
    #[getset(get_clone = "pub(crate)")]
    listener_ref: ListenerRef,
    #[getset(get_clone = "pub(crate)")]
    client: Arc<WsClient<RpcLogger<Tracing<RpcService>>>>,
}

#[derive(Debug, TypedBuilder)]
pub struct ConfigurationTransportOptions {
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
impl AsyncTryFrom<ConfigurationTransportOptions> for ConfigurationTransport {
    type Error = ConfigurationClientInitError;

    async fn async_try_from(value: ConfigurationTransportOptions) -> Result<Self, Self::Error> {
        let rpc_middleware = RpcServiceBuilder::default()
            .rpc_logger(0)
            .layer(TracingLayer::default());

        let client = WsClientBuilder::new()
            .set_rpc_middleware(rpc_middleware)
            .build(value.address.to_string())
            .await
            .map_err(|err| {
                println!("error creating ws client: {:?}", err);
                ConfigurationClientInitError::WsClientError
            })?;

        let transport = ConfigurationTransport::builder()
            .listener_ref(value.listener_ref)
            .client(Arc::new(client))
            .build();

        Ok(transport)
    }
}
