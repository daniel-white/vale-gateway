use crate::handler::static_response::body::BodyContent;
use crate::handler::static_response::handler::StaticResponseFilterHandler;
use crate::stage::inbound_request::InboundRequestFilterHandler;
use http::StatusCode;
use std::sync::Arc;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::filter::static_response::StaticResponseFilter;

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct StaticResponseFilterHandlerLayer {
    status_code: StatusCode,
    body_content: Arc<Option<BodyContent>>,
}

impl Layer<InboundRequestFilterHandler> for StaticResponseFilterHandlerLayer {
    type Service = StaticResponseFilterHandler;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .status_code(self.status_code)
            .body_content(self.body_content.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum StaticResponseFilterHandlerLayerError {}

impl TryFrom<&StaticResponseFilter> for StaticResponseFilterHandlerLayer {
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
