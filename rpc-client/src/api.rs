use crate::instrumentation::TRACER;
use jsonrpsee::core::ClientError as JsonrpseeClientError;
use opentelemetry::trace::{SpanKind, Tracer};
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ApiClient, ApiError, GetBackendRequest, GetListenerRequest,
    GetRouteRequest, GetSharedFilterRequest, RequestContext,
};
use crate::transport::Transport;

#[derive(Clone, TypedBuilder)]
pub struct Client {
    transport: Transport,
}

impl Client {
    pub async fn listener(&self) -> Result<Arc<Listener>, ClientError> {
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
    ) -> Result<Arc<Route>, ClientError> {
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
    ) -> Result<Arc<Backend>, ClientError> {
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
    ) -> Result<Arc<SharedFilter>, ClientError> {
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
pub enum ClientError {
    #[error("Listener not found")]
    NotFound,
    #[error("Request timeout")]
    RequestTimeout,
    #[error("Unknown error")]
    Unknown,
}

impl From<JsonrpseeClientError> for ClientError {
    fn from(value: JsonrpseeClientError) -> Self {
        match value {
            JsonrpseeClientError::Call(err) => match ApiError::from(err) {
                ApiError::NotFound => ClientError::NotFound,
                _ => ClientError::Unknown,
            },
            JsonrpseeClientError::RequestTimeout => ClientError::RequestTimeout,
            _ => ClientError::Unknown,
        }
    }
}
