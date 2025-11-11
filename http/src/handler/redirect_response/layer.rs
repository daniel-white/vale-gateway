use crate::handler::redirect_response::handler::RedirectResponseFilterHandler;
use crate::rewriting::uri::{UriRewriter, UriRewriterConversionError};
use crate::stage::inbound_request::InboundRequestFilterHandler;
use http::StatusCode;
use std::sync::Arc;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::filter::redirect_response::RedirectResponseFilter;

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct RedirectResponseFilterHandlerLayer {
    status_code: StatusCode,
    rewriter: Arc<UriRewriter>,
}

impl Layer<InboundRequestFilterHandler> for RedirectResponseFilterHandlerLayer {
    type Service = RedirectResponseFilterHandler;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        RedirectResponseFilterHandler::builder()
            .inner(inner)
            .status_code(self.status_code)
            .rewriter(self.rewriter.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum RedirectResponseFilterHandlerLayerError {
    #[error("Status code not in redirect range")]
    StatusCode,
    #[error(transparent)]
    Rewriter(#[from] UriRewriterConversionError),
}

impl TryFrom<&RedirectResponseFilter> for RedirectResponseFilterHandlerLayer {
    type Error = RedirectResponseFilterHandlerLayerError;

    fn try_from(value: &RedirectResponseFilter) -> Result<Self, Self::Error> {
        if !value.status_code().is_redirection() {
            return Err(Self::Error::StatusCode);
        }

        let rewriter: UriRewriter = value.uri().try_into()?;

        let layer = Self::builder()
            .status_code(value.status_code())
            .rewriter(Arc::new(rewriter))
            .build();

        Ok(layer)
    }
}
