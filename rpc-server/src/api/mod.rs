use std::net::SocketAddr;
use std::sync::Arc;
use derive_more::From;
use jsonrpsee::server::{Server, ServerHandle};
use typed_builder::TypedBuilder;
use vg_config::provider::DataProvider;
use vg_rpc::ApiServer as ApiServerT;
use crate::api::error::ApiServerStartError;
use crate::api::methods::ApiServerMethods;
use crate::events::sinks::EventSinkRegistry;

pub mod error;
mod methods;

#[derive(TypedBuilder)]
pub struct ApiServerOptions {
    #[builder(setter(into))]
    binding: SocketAddr,
    event_sinks: EventSinkRegistry,
    data_provider: Arc<dyn DataProvider>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ApiServer {
    binding: SocketAddr,
    methods: ApiServerMethods
}

impl From<ApiServerOptions> for ApiServer {
    fn from(value: ApiServerOptions) -> Self {
        let methods = ApiServerMethods::builder()
            .data_provider(value.data_provider)
            .event_sinks(value.event_sinks)
            .build();

        Self::builder()
            .binding(value.binding)
            .methods(methods)
            .build()
    }
}


#[derive(From, Clone)]
pub struct ApiServerStopHandle(ServerHandle);

impl ApiServerStopHandle {
    pub async fn stopped(self) {
        self.0.stopped().await
    }
}


impl ApiServer {
    pub async fn start(self) -> Result<ApiServerStopHandle, ApiServerStartError> {
        let server = Server::builder()
            .build(self.binding)
            .await
            .map_err(|_| ApiServerStartError::Unknown)?;
        
        Ok(server.start(self.methods.into_rpc()).into())
    }
}
