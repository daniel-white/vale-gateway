use crate::handler::static_response::body::BodyContent;
use crate::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult,
};
use bytes::Bytes;
use futures::future::BoxFuture;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::request::Parts;
use http::{HeaderValue, StatusCode};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct StaticResponseFilterHandler {
    inner: InboundRequestFilterHandler,
    status_code: StatusCode,
    body_content: Arc<Option<BodyContent>>,
}

impl Service<Parts> for StaticResponseFilterHandler {
    type Response = InboundRequestFilterResult;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO load remote body content
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Parts) -> Self::Future {
        let status_code = self.status_code;
        let body_content = self.body_content.clone();
        Box::pin(async move {
            let builder = http::Response::builder().status(status_code);

            let response = match body_content.as_ref() {
                Some(body) => {
                    let content_type: HeaderValue = body.content_type().try_into().expect("Invalid content type");
                    let body = Bytes::from(body.data().clone());

                    builder
                        .header(CONTENT_TYPE, content_type)
                        .header(CONTENT_LENGTH, body.len())
                        .body(Some(body))
                        .expect("unable to build response")
                }
                None => builder.body(None).expect("unable to build response"),
            };

            Ok(response.into())
        })
    }
}
