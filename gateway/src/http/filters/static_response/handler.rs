use crate::http::filters::static_response::cache::HttpStaticResponseFilterBodyCacheClient;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http::{HeaderValue, Response, StatusCode};
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_core::http::filters::static_response::{HttpStaticResponseBody, HttpStaticResponseBodyKey, HttpStaticResponseFilter, HttpStaticResponseFilterKey};

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpStaticResponseFilterHandler {
    key: HttpStaticResponseFilterKey,
    status_code: StatusCode,
    body: Option<HttpStaticResponseFilterHandlerBody>,
}

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpStaticResponseFilterHandlerBody {
    key: HttpStaticResponseBodyKey,
    content_type: HeaderValue,
    client: HttpStaticResponseFilterBodyCacheClient,
}

impl HttpStaticResponseFilterHandler {
    pub fn from(filter: &HttpStaticResponseFilter, body: Option<HttpStaticResponseFilterHandlerBody>) -> Self {
        
        HttpStaticResponseFilterHandler::builder()
            .key(filter.key().clone())
            .status_code(*filter.status_code())
            .body(body)
            .build()
    }


    pub async fn generate_response(&self) -> Response<Option<Arc<Bytes>>> {
        let mut response = Response::builder().status(self.status_code);

        let response = match self.body.as_ref() {
            Some(body) => {
                response = response.header(CONTENT_TYPE, &body.content_type);
                let body = body.client.get(&body.key).await;
                response.body(body)
            }
            None => response.body(None),
        };

        response.expect("Failed to build response")
    }
}

impl HttpStaticResponseFilterHandlerBody {
    pub fn from(
        body: &HttpStaticResponseBody,
        client: HttpStaticResponseFilterBodyCacheClient,
    ) -> Self {
        Self::builder()
            .key(body.key().clone())
            .content_type(body.content_type().clone())
            .client(client)
            .build()
    }
}
