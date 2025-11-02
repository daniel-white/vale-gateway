use std::sync::Arc;
use crate::extensions::routing::RequestMatch;
use crate::rewriting::uri::{UriRewriter, UriRewriterConversionError};
use http::request::Parts;
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use thiserror::Error;
use tower::{Layer, Service};
use typed_builder::TypedBuilder;
use vg_config::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use crate::filter::backend_request::{BackendRequestFilterError, DynBackendRequestFilter};

#[derive(Debug, Clone, TypedBuilder)]
pub struct BackendUriRewriterFilterHandler {
    inner: DynBackendRequestFilter,
    rewriter: Arc<UriRewriter>,
}

impl Service<Parts> for BackendUriRewriterFilterHandler {
    type Response = ();
    type Error = BackendRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Parts) -> Self::Future {
        let request_match = req
            .extensions
            .get::<RequestMatch>()
            .expect("missing request match");

        let new_uri = self.rewriter.rewrite(&req.uri, request_match);
        req.uri = new_uri;

        self.inner.call(req)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct BackendUriRewriterFilterHandlerLayer {
    rewriter: Arc<UriRewriter>,
}

impl Layer<DynBackendRequestFilter> for BackendUriRewriterFilterHandlerLayer {
    type Service = BackendUriRewriterFilterHandler;

    fn layer(&self, inner: DynBackendRequestFilter) -> Self::Service {
        BackendUriRewriterFilterHandler::builder()
            .inner(inner)
            .rewriter(self.rewriter.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum BackendUriRewriterFilterHandlerLayerError {
    #[error(transparent)]
    Rewriter(#[from] UriRewriterConversionError),
}

impl TryFrom<&BackendUriRewriterFilter> for BackendUriRewriterFilterHandlerLayer
{
    type Error = BackendUriRewriterFilterHandlerLayerError;

    fn try_from(value: &BackendUriRewriterFilter) -> Result<Self, Self::Error> {
        let rewriter: UriRewriter = value.uri().try_into()?;

        let layer = Self::builder().rewriter(Arc::new(rewriter)).build();

        Ok(layer)
    }
}
