use crate::filter::backend_request::{BackendRequestFilterError, DynBackendRequestFilter};
use crate::filter::inbound_request::{DynInboundRequestFilter, InboundRequestFilterError, InboundRequestFilterResponse};
use crate::filter::response::{DynResponseFilter, ResponseFilterError};
use futures::future::BoxFuture;
use http::{HeaderMap, HeaderName};
use std::collections::HashSet;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tower::{Layer, Service};
use typed_builder::TypedBuilder;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;

#[derive(Debug, Clone, TypedBuilder)]
pub struct HeaderModifierFilterHandler<S> {
    inner: S,
    modifier: Arc<HeaderMapModifier>,
}

impl Service<http::request::Parts> for HeaderModifierFilterHandler<DynInboundRequestFilter> {
    type Response = InboundRequestFilterResponse;
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

impl Service<http::request::Parts> for HeaderModifierFilterHandler<DynBackendRequestFilter> {
    type Response = ();
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

impl Service<http::response::Parts> for HeaderModifierFilterHandler<DynResponseFilter> {
    type Response = ();
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
struct HeaderMapModifier {
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

#[derive(Debug, TypedBuilder)]
pub struct HeaderModifierFilterHandlerLayer {
    modifier: Arc<HeaderMapModifier>,
}

impl Layer<DynInboundRequestFilter> for HeaderModifierFilterHandlerLayer {
    type Service = HeaderModifierFilterHandler<DynInboundRequestFilter>;

    fn layer(&self, inner: DynInboundRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .modifier(self.modifier.clone())
            .build()
    }
}

impl Layer<DynBackendRequestFilter> for HeaderModifierFilterHandlerLayer {
    type Service = HeaderModifierFilterHandler<DynBackendRequestFilter>;

    fn layer(&self, inner: DynBackendRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .modifier(self.modifier.clone())
            .build()
    }
}

impl Layer<DynResponseFilter> for HeaderModifierFilterHandlerLayer {
    type Service = HeaderModifierFilterHandler<DynResponseFilter>;

    fn layer(&self, inner: DynResponseFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .modifier(self.modifier.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum HeaderModifierFilterHandlerLayerError {}

impl TryFrom<&HeaderModifierFilter> for HeaderModifierFilterHandlerLayer {
    type Error = HeaderModifierFilterHandlerLayerError;

    fn try_from(value: &HeaderModifierFilter) -> Result<Self, Self::Error> {
        let modifier = HeaderMapModifier::builder()
            .add(value.add())
            .set(value.set())
            .remove(value.remove().iter().cloned().collect())
            .build();

        let layer = Self::builder().modifier(Arc::new(modifier)).build();

        Ok(layer)
    }
}

