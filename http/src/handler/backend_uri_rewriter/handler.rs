use crate::extensions::routing::RequestMatch;
use crate::rewriting::uri::UriRewriter;
use crate::stage::backend_request::{
    BackendRequestFilter, BackendRequestFilterError, BackendRequestFilterResult,
};
use futures::future::BoxFuture;
use http::request::Parts;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct BackendUriRewriterFilterHandler {
    inner: BackendRequestFilter,
    rewriter: Arc<UriRewriter>,
}

impl Service<Parts> for BackendUriRewriterFilterHandler {
    type Response = BackendRequestFilterResult;
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
