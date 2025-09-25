use async_trait::async_trait;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http::{HeaderValue, Response, StatusCode};
use std::fmt::Debug;
use std::sync::Arc;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseFilterHandler {
    status_code: StatusCode,
    body_resolver: Option<Box<dyn StaticResponseBodyResolver>>,
}

#[async_trait]
pub trait StaticResponseBodyResolver: Debug {
    async fn resolve_body(&self) -> Option<StaticResponseBody>;
}

#[derive(Debug)]
pub struct EmptyStaticResponseBodyResolver;

#[async_trait]
impl StaticResponseBodyResolver for EmptyStaticResponseBodyResolver {
    async fn resolve_body(&self) -> Option<StaticResponseBody> {
        None
    }
}

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseBody {
    content_type: HeaderValue,
    content: Arc<Bytes>,
}

impl StaticResponseFilterHandler {
    pub async fn generate_response(&self) -> Response<Option<Arc<Bytes>>> {
        let mut response = Response::builder().status(self.status_code);

        let body = match &self.body_resolver {
            Some(body_resolver) => body_resolver.resolve_body().await,
            None => None,
        };

        let response = match body {
            Some(body) => {
                response = response.header(CONTENT_TYPE, &body.content_type);
                response.body(Some(body.content))
            }
            None => response.body(None),
        };

        response.expect("Failed to build static response")
    }
}
