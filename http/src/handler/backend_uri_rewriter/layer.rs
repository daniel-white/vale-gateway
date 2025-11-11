use crate::handler::backend_uri_rewriter::handler::BackendUriRewriterFilterHandler;
use crate::rewriting::uri::{UriRewriter, UriRewriterConversionError};
use crate::stage::backend_request::BackendRequestFilter;
use std::sync::Arc;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct BackendUriRewriterFilterHandlerLayer {
    rewriter: Arc<UriRewriter>,
}

impl TryFrom<&BackendUriRewriterFilter> for BackendUriRewriterFilterHandlerLayer {
    type Error = BackendUriRewriterFilterHandlerLayerError;

    fn try_from(value: &BackendUriRewriterFilter) -> Result<Self, Self::Error> {
        let rewriter: UriRewriter = value.uri().try_into()?;

        let layer = Self::builder().rewriter(Arc::new(rewriter)).build();

        Ok(layer)
    }
}

impl Layer<BackendRequestFilter> for BackendUriRewriterFilterHandlerLayer {
    type Service = BackendUriRewriterFilterHandler;

    fn layer(&self, inner: BackendRequestFilter) -> Self::Service {
        BackendUriRewriterFilterHandler::builder()
            .inner(inner)
            .rewriter(self.rewriter.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum BackendUriRewriterFilterHandlerLayerError {
    #[error(transparent)]
    UriRewriter(#[from] UriRewriterConversionError),
}
