use crate::extensions::routing::RequestMatch;
use crate::rewriting::uri::UriRewriter;
use crate::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult,
};
use futures::future::BoxFuture;
use http::header::LOCATION;
use http::request::Parts;
use http::{Response, StatusCode};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct RedirectResponseFilterHandler {
    inner: InboundRequestFilterHandler,
    status_code: StatusCode,
    rewriter: Arc<UriRewriter>,
}

impl Service<Parts> for RedirectResponseFilterHandler {
    type Response = InboundRequestFilterResult;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Parts) -> Self::Future {
        let status_code = self.status_code;
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
            Ok(InboundRequestFilterResult::Respond(response))
        })
    }
}
