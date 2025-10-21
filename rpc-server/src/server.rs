use crate::EventSinkRegistry;
use crate::api::ApiServerImpl;
use derive_more::From;
use jsonrpsee::server::{Server, ServerHandle};
use std::net::SocketAddr;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::provider::HttpConfigurationProvider;
use vg_rpc::{ApiServer};

#[derive(TypedBuilder)]
pub struct ApiServerOptions {
    #[builder(setter(into))]
    binding: SocketAddr,
    event_sinks: EventSinkRegistry,
    http_configuration: Box<dyn HttpConfigurationProvider>,
}

#[derive(Debug, Error)]
#[error("TODO server error")]
pub struct StartApiServerError;

#[derive(From, Clone)]
pub struct ApiServerHandle(ServerHandle);

impl ApiServerHandle {
    pub async fn stopped(self) {
        self.0.stopped().await
    }
}

impl ApiServerOptions {
    pub async fn start_server(
        self,
    ) -> Result<ApiServerHandle, StartApiServerError> {
        let server = Server::builder()
            .build(self.binding)
            .await
            .map_err(|_| StartApiServerError)?;

        let api_server = ApiServerImpl::builder()
            .event_sinks(self.event_sinks)
            .http_configuration(self.http_configuration)
            .build();

        Ok(server.start(api_server.into_rpc()).into())
    }
}
