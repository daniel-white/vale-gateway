pub  mod body;

use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::request::Parts;
use http::{HeaderValue, StatusCode};
use std::sync::Arc;
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use thiserror::Error;
use tower::{Layer, Service};
use typed_builder::TypedBuilder;
use body::BodyContent;
use vg_config::http::filter::static_response::StaticResponseFilter;
use crate::filter::inbound_request::{DynInboundRequestFilter, InboundRequestFilterError, InboundRequestFilterResponse};

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseFilterHandler {
    inner: DynInboundRequestFilter,
    status_code: StatusCode,
    body_content: Arc<Option<BodyContent>>,
}

impl Service<Parts> for StaticResponseFilterHandler {
    type Response = InboundRequestFilterResponse;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO load remote body content
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Parts) -> Self::Future {
        Box::pin(async move {
            let mut builder = http::Response::builder().status(self.status_code);

            let response = match self.body_content.as_ref() {
                Some(body) => {
                    let content_type: HeaderValue =
                        body.content_type().try_into().expect("Invalid content type");
                    let body = Bytes::from(body.data().to_vec());

                    builder
                        .header(CONTENT_TYPE, content_type)
                        .header(CONTENT_LENGTH, body.len())
                        .body(Some(body))
                        .expect("unable to build response")
                }
                None => builder.body(None).expect("unable to build response"),
            };

            let response = InboundRequestFilterResponse::Respond(response);

            Ok(response)
        })
    }
}

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseFilterHandlerLayer {
    status_code: StatusCode,
    body_content: Arc<Option<BodyContent>>,
}

impl Layer<DynInboundRequestFilter> for StaticResponseFilterHandlerLayer {
    type Service = StaticResponseFilterHandler;

    fn layer(&self, inner: DynInboundRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .status_code(self.status_code)
            .body_content(self.body_content.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum StaticResponseFilterHandlerLayerError {}

impl TryFrom<&StaticResponseFilter> for StaticResponseFilterHandlerLayer
{
    type Error = StaticResponseFilterHandlerLayerError;

    fn try_from(value: &StaticResponseFilter) -> Result<Self, Self::Error> {
        let body_content = value.body().as_ref().map(BodyContent::from);
        let layer = Self::builder()
            .status_code(value.status_code())
            .body_content(Arc::new(body_content))
            .build();

        Ok(layer)
    }
}
