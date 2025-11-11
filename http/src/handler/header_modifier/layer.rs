use crate::handler::header_modifier::handler::{HeaderMapModifier, HeaderModifierFilterHandler};
use crate::stage::backend_request::BackendRequestFilter;
use crate::stage::inbound_request::InboundRequestFilterHandler;
use crate::stage::response::ResponseFilter;
use std::sync::Arc;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct HeaderModifierFilterHandlerLayer {
    modifier: Arc<HeaderMapModifier>,
}

impl Layer<InboundRequestFilterHandler> for HeaderModifierFilterHandlerLayer {
    type Service = HeaderModifierFilterHandler<InboundRequestFilterHandler>;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .modifier(self.modifier.clone())
            .build()
    }
}

impl Layer<BackendRequestFilter> for HeaderModifierFilterHandlerLayer {
    type Service = HeaderModifierFilterHandler<BackendRequestFilter>;

    fn layer(&self, inner: BackendRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .modifier(self.modifier.clone())
            .build()
    }
}

impl Layer<ResponseFilter> for HeaderModifierFilterHandlerLayer {
    type Service = HeaderModifierFilterHandler<ResponseFilter>;

    fn layer(&self, inner: ResponseFilter) -> Self::Service {
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
