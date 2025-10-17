use crate::ConfigurationTransport;
use crate::instrumentation::TRACER;
use jsonrpsee::core::ClientError;
use opentelemetry::trace::{SpanKind, Tracer};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ConfigurationApiClient, ConfigurationApiError, RequestContext, GetBackendRequest, GetListenerRequest,
    GetRouteRequest,
};

#[derive(Clone, TypedBuilder)]
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Listener, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let client = self.transport.client();
        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport.listener_ref())
            .build();

        Ok(client.listener(req).await?)
    }

    pub async fn route(&self, route_ref: &RouteRef) -> Result<Route, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::route")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let client = self.transport.client();
        let req = GetRouteRequest::builder()
            .context(RequestContext::new(span))
            .route_ref(route_ref.clone())
            .build();

        Ok(client.route(req).await?)
    }

    pub async fn backend(
        &self,
        backend_ref: &BackendRef,
    ) -> Result<Backend, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::backend")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let client = self.transport.client();
        let req = GetBackendRequest::builder()
            .context(RequestContext::new(span))
            .backend_ref(backend_ref.clone())
            .build();

        Ok(client.backend(req).await?)
    }
}

#[derive(Debug, Clone, Error)]
pub enum ConfigurationClientError {
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Unknown client")]
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
