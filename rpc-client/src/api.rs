use crate::ConfigurationTransport;
use jsonrpsee::core::ClientError;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{ConfigurationApiClient, ConfigurationApiError};

#[derive(Clone, TypedBuilder)]
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Listener, ConfigurationClientError> {
        let client = self.transport.client();
        let listener_ref = self.transport.listener_ref();

        Ok(client.listener(listener_ref).await?)
    }

    pub async fn route(&self, route_ref: &RouteRef) -> Result<Route, ConfigurationClientError> {
        let client = self.transport.client();
        let route_ref = route_ref.clone();

        Ok(client.route(route_ref).await?)
    }

    pub async fn backend(
        &self,
        backend_ref: &BackendRef,
    ) -> Result<Backend, ConfigurationClientError> {
        let client = self.transport.client();
        let backend_ref = backend_ref.clone();

        Ok(client.backend(backend_ref).await?)
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationClientError {
    #[error("Listener not found")]
    NotFound(#[source] ClientError),
    #[error("Request timeout")]
    RequestTimeout(#[source] ClientError),
    #[error("Unknown")]
    Unknown,
}

impl From<ClientError> for ConfigurationClientError {
    fn from(value: ClientError) -> Self {
        match &value {
            ClientError::Call(err) => match ConfigurationApiError::from(err) {
                ConfigurationApiError::NotFound => ConfigurationClientError::NotFound(value),
                _ => ConfigurationClientError::Unknown,
            },
            ClientError::RequestTimeout => ConfigurationClientError::RequestTimeout(value),
            _ => ConfigurationClientError::Unknown,
        }
    }
}
