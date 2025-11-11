use crate::stage::backend_request::{
    BackendRequestFilter, BackendRequestFilterError, BackendRequestFilterResult,
};
use crate::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult,
};
use crate::stage::response::{ResponseFilter, ResponseFilterError, ResponseFilterResult};
use futures::future::BoxFuture;
use http::{HeaderMap, HeaderName};
use std::collections::HashSet;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct HeaderModifierFilterHandler<S> {
    inner: S,
    modifier: Arc<HeaderMapModifier>,
}

impl Service<http::request::Parts> for HeaderModifierFilterHandler<InboundRequestFilterHandler> {
    type Response = InboundRequestFilterResult;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::request::Parts) -> Self::Future {
        self.modifier.apply(&mut req.headers);
        self.inner.call(req)
    }
}

impl Service<http::request::Parts> for HeaderModifierFilterHandler<BackendRequestFilter> {
    type Response = BackendRequestFilterResult;
    type Error = BackendRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::request::Parts) -> Self::Future {
        self.modifier.apply(&mut req.headers);
        self.inner.call(req)
    }
}

impl Service<http::response::Parts> for HeaderModifierFilterHandler<ResponseFilter> {
    type Response = ResponseFilterResult;
    type Error = ResponseFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::response::Parts) -> Self::Future {
        self.modifier.apply(&mut req.headers);
        self.inner.call(req)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct HeaderMapModifier {
    add: HeaderMap,
    set: HeaderMap,
    remove: HashSet<HeaderName>,
}

impl HeaderMapModifier {
    pub fn apply(&self, headers: &mut HeaderMap) {
        // Remove headers first
        for name in self.remove.iter() {
            headers.remove(name);
        }

        // Set headers (overwrite existing)
        for (name, value) in self.set.iter() {
            headers.insert(name, value.clone());
        }

        // Add headers (append to existing)
        for (name, value) in self.add.iter() {
            headers.append(name, value.clone());
        }
    }
}
