use crate::instrumentation::TRACER;
use crate::transport::{Client, TransportClient};
use opentelemetry::trace::{SpanKind, Tracer};
use std::sync::Arc;
use typed_builder::TypedBuilder;
use error::ApiClientError;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc::{
    ApiClient as ApiClientTrait, GetBackendRequest, GetListenerRequest, GetRouteRequest,
    GetSharedFilterRequest, RequestContext,
};

pub mod error;

#[derive(TypedBuilder)]
pub struct ApiClientOptions {
    transport_client: TransportClient,
}

#[derive(Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct ApiClient {
    transport_client: TransportClient,
}

impl ApiClient {
    pub async fn listener(&self) -> Result<Arc<Listener>, ApiClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let Client::Connected(transport_client) = self.transport_client.client() else {
            return Err(ApiClientError::ServiceUnavailable);
        };

        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.transport_client.listener_ref())
            .build();

        let listener = transport_client.listener(req).await?;

        Ok(Arc::new(listener))
    }

    pub async fn route(&self, route_ref: &RouteRef) -> Result<Arc<Route>, ApiClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::route")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let Client::Connected(transport_client) = self.transport_client.client() else {
            return Err(ApiClientError::ServiceUnavailable);
        };

        let req = GetRouteRequest::builder()
            .context(RequestContext::new(span))
            .route_ref(route_ref.clone())
            .build();

        let route = transport_client.route(req).await?;

        Ok(Arc::new(route))
    }

    pub async fn backend(&self, backend_ref: &BackendRef) -> Result<Arc<Backend>, ApiClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::backend")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let Client::Connected(transport_client) = self.transport_client.client() else {
            return Err(ApiClientError::ServiceUnavailable);
        };

        let req = GetBackendRequest::builder()
            .context(RequestContext::new(span))
            .backend_ref(backend_ref.clone())
            .build();

        let backend = transport_client.backend(req).await?;

        Ok(Arc::new(backend))
    }

    pub async fn shared_filter(
        &self,
        filter_ref: &SharedFilterRef,
    ) -> Result<Arc<SharedFilter>, ApiClientError> {
        let span = TRACER
            .span_builder("ConfigurationClient::shared_filter")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let Client::Connected(transport_client) = self.transport_client.client() else {
            return Err(ApiClientError::ServiceUnavailable);
        };

        let req = GetSharedFilterRequest::builder()
            .context(RequestContext::new(span))
            .filter_ref(filter_ref.clone())
            .build();

        let filter = transport_client.shared_filter(req).await?;

        Ok(Arc::new(filter))
    }
}

impl From<ApiClientOptions> for ApiClient {
    fn from(value: ApiClientOptions) -> Self {
        Self::builder()
            .transport_client(value.transport_client)
            .build()
    }
}

