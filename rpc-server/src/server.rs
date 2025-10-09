use crate::ConfigurationEventManager;
use crate::methods::ConfigurationApiServerMethods;
use derive_more::From;
use jsonrpsee::server::{Server, ServerHandle};
use std::net::SocketAddr;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::provider::HttpConfigurationProvider;
use vg_rpc::ConfigurationApiServer;

#[derive(TypedBuilder)]
pub struct ConfigurationServerOptions {
    #[builder(setter(into))]
    binding: SocketAddr,
    event_manager: ConfigurationEventManager,
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
        let server = Server::builder()
            .build(self.binding)
            .await
            .map_err(|_| StartConfigurationServerError)?;

        let methods = ConfigurationApiServerMethods::builder()
            .event_manager(self.event_manager)
            .http_configuration(self.http_configuration)
            .build();

        Ok(server.start(methods.into_rpc()).into())
    }
}
