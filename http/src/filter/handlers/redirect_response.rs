use std::sync::Arc;
use crate::extensions::routing::RequestMatch;
use crate::rewriting::uri::{UriRewriter, UriRewriterConversionError};
use http::header::LOCATION;
use http::request::Parts;
use http::{Response, StatusCode};
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use thiserror::Error;
use tower::{Layer, Service};
use typed_builder::TypedBuilder;
use vg_config::http::filter::redirect_response::RedirectResponseFilter;
use crate::filter::inbound_request::{DynInboundRequestFilter, InboundRequestFilterError, InboundRequestFilterResponse};

#[derive(Debug, Clone, TypedBuilder)]
pub struct RedirectResponseFilterHandler {
    inner: DynInboundRequestFilter,
    status_code: StatusCode,
    rewriter: Arc<UriRewriter>,
}


impl Service<Parts> for RedirectResponseFilterHandler {
    type Response = InboundRequestFilterResponse;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Parts) -> Self::Future {
        let status_code = self.status_code.clone();
        let rewriter = self.rewriter.clone();
        
        Box::pin(async move {
            let request_match = req
                .extensions
                .get::<RequestMatch>()
                .expect("missing request match");

            let redirect_uri = rewriter.rewrite(&req.uri, request_match);

            let response = Response::builder()
                .status(status_code)
                .header(LOCATION, redirect_uri.to_string())
                .body(None)
                .expect("unable to build redirect response");
            Ok(InboundRequestFilterResponse::Respond(response))
        })
    }
}

#[derive(Debug, Clone, TypedBuilder)]
pub struct RedirectResponseFilterHandlerLayer {
    status_code: StatusCode,
    rewriter: Arc<UriRewriter>,
}

impl Layer<DynInboundRequestFilter> for RedirectResponseFilterHandlerLayer {
    type Service = RedirectResponseFilterHandler;

    fn layer(&self, inner: DynInboundRequestFilter) -> Self::Service {
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

impl TryFrom<&RedirectResponseFilter> for RedirectResponseFilterHandlerLayer
{
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
