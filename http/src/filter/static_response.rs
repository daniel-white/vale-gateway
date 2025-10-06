use async_trait::async_trait;
use getset::Getters;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE, InvalidHeaderValue};
use http::{HeaderValue, Response, StatusCode, Uri};
use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_core::http::content_type::ContentTypeBuf;
use vg_http_config::filter::static_response::{
    Body as BodyConfig, BodyContent as BodyContentConfig, StaticResponseFilter,
};

#[derive(Debug, Error)]
pub enum BodyResolverError {
    #[error("Body not found")]
    NotFound,
    #[error("Unspecified error")]
    Unspecified,
}

#[async_trait]
pub trait BodyResolver {
    async fn resolve(&self, ref_: &BodyRef) -> Result<Body, BodyResolverError>;
}

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseFilterHandler {
    status_code: StatusCode,
    body_ref: Option<BodyRef>,
}

#[derive(Debug, Error)]
pub enum StaticResponseFilterHandlerConversionError {
    #[error("Body ref error: {0}")]
    BodyRef(
        #[from]
        #[source]
        BodyRefConversionError,
    ),
}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&StaticResponseFilter> for StaticResponseFilterHandler {
    type Error = StaticResponseFilterHandlerConversionError;

    fn try_from(value: &StaticResponseFilter) -> Result<Self, Self::Error> {
        let body_ref = match &value.body() {
            Some(body) => Some(body.try_into()?),
            None => None,
        };

        let handler = StaticResponseFilterHandler::builder()
            .status_code(value.status_code())
            .body_ref(body_ref)
            .build();

        Ok(handler)
    }
}

#[derive(Debug, Error)]
pub enum StaticResponseFilterError {
    #[error("Body resolution error: {0}")]
    BodyResolutionError(#[from] BodyResolverError),
    #[error("Invalid content type: {0}")]
    InvalidContentType(#[from] InvalidHeaderValue),
    #[error("Response is invalid: {0}")]
    InvalidResponse(#[from] http::Error),
}

impl StaticResponseFilterHandler {
    pub async fn generate_response<R: BodyResolver>(
        &self,
        body_resolver: &R,
    ) -> Result<Response<Option<Arc<[u8]>>>, StaticResponseFilterError> {
        let builder = Response::builder().status(self.status_code);

        let response = match self.body_ref.as_ref() {
            Some(body) => {
                let body = body_resolver.resolve(body).await?;
                let content_type: HeaderValue = (&body.content_type).try_into()?;
                builder
                    .header(CONTENT_TYPE, content_type)
                    .header(CONTENT_LENGTH, body.content.len())
                    .body(Some(body.content))?
            }
            None => builder.body(None)?,
        };

        Ok(response)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct Body {
    content_type: ContentTypeBuf,
    content: Arc<[u8]>,
}

#[derive(Debug, TypedBuilder, Getters)]
pub struct BodyRef {
    #[getset(get = "pub")]
    content_type: ContentTypeBuf,
    #[getset(get = "pub")]
    content: BodyRefContent,
}

#[derive(Debug, Error)]
pub enum BodyRefConversionError {}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&BodyConfig> for BodyRef {
    type Error = BodyRefConversionError;

    fn try_from(value: &BodyConfig) -> Result<Self, Self::Error> {
        let content = match &value.content() {
            BodyContentConfig::Text(text) => BodyRefContent::Local(Arc::from(text.as_bytes())),
            BodyContentConfig::Binary(data) => BodyRefContent::Local(data.clone()),
            BodyContentConfig::Remote(uri) => BodyRefContent::Remote(uri.clone()),
        };

        let ref_ = BodyRef::builder()
            .content_type(value.content_type().clone())
            .content(content)
            .build();

        Ok(ref_)
    }
}

#[derive(Debug)]
pub enum BodyRefContent {
    Local(Arc<[u8]>),
    Remote(Uri),
}
