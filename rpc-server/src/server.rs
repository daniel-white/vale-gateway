use crate::ConfigurationEventSinkRegistry;
use crate::api::ConfigurationApiServerMethods;
use crate::middleware::tracing::TracingLayer;
use derive_more::From;
use jsonrpsee::server::{Server, ServerHandle};
use jsonrpsee_core::middleware::RpcServiceBuilder;
use std::net::SocketAddr;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::provider::HttpConfigurationProvider;
use vg_rpc::ConfigurationApiServer;

#[derive(TypedBuilder)]
pub struct ConfigurationServerOptions {
    #[builder(setter(into))]
    binding: SocketAddr,
    event_sinks: ConfigurationEventSinkRegistry,
    http_configuration: Box<dyn HttpConfigurationProvider>,
}

#[derive(Debug, Error)]
#[error("TODO server error")]
pub struct StartConfigurationServerError;

#[derive(From, Clone)]
pub struct ConfigurationServerHandle(ServerHandle);

impl ConfigurationServerHandle {
    pub async fn stopped(self) {
        self.0.stopped().await
    }
}

impl ConfigurationServerOptions {
    pub async fn start_server(
        self,
    ) -> Result<ConfigurationServerHandle, StartConfigurationServerError> {
        let rpc_middleware = RpcServiceBuilder::default()
            .layer(TracingLayer)
            .rpc_logger(0)
;

        let server = Server::builder()
            .set_rpc_middleware(rpc_middleware)
            .build(self.binding)
            .await
            .map_err(|_| StartConfigurationServerError)?;

        let methods = ConfigurationApiServerMethods::builder()
            .event_sinks(self.event_sinks)
            .http_configuration(self.http_configuration)
            .build();

        Ok(server.start(methods.into_rpc()).into())
    }
}
