use crate::ConfigurationTransport;
use crate::instrumentation::TRACER;
use jsonrpsee::core::ClientError;
use opentelemetry::trace::{SpanKind, Tracer};
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ConfigurationApiClient, ConfigurationApiError, GetBackendRequest, GetListenerRequest,
    GetRouteRequest, GetSharedFilterRequest, RequestContext,
};

#[derive(Clone, TypedBuilder)]
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Arc<Listener>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let client = self.transport.client();
        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport.listener_ref())
            .build();

        let listener = client.listener(req).await?;

        Ok(Arc::new(listener))
    }

    pub async fn route(
        &self,
        route_ref: &RouteRef,
    ) -> Result<Arc<Route>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::route")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let client = self.transport.client();
        let req = GetRouteRequest::builder()
            .context(RequestContext::new(span))
            .route_ref(route_ref.clone())
            .build();

        let route = client.route(req).await?;

        Ok(Arc::new(route))
    }

    pub async fn backend(
        &self,
        backend_ref: &BackendRef,
    ) -> Result<Arc<Backend>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::backend")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let client = self.transport.client();
        let req = GetBackendRequest::builder()
            .context(RequestContext::new(span))
            .backend_ref(backend_ref.clone())
            .build();

        let backend = client.backend(req).await?;

        Ok(Arc::new(backend))
    }

    pub async fn shared_filter(
        &self,
        filter_ref: &SharedFilterRef,
    ) -> Result<Arc<SharedFilter>, ConfigurationClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::shared_filter")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
        let client = self.transport.client();
        let req = GetSharedFilterRequest::builder()
            .context(RequestContext::new(span))
            .filter_ref(filter_ref.clone())
            .build();

        let filter = client.shared_filter(req).await?;

        Ok(Arc::new(filter))
    }
}

#[derive(Debug, Clone, Error)]
pub enum ConfigurationClientError {
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Unknown error")]
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
