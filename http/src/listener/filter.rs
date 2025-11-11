use crate::filter::SharedFilterHandlerLayer;
use crate::handler::{
    AccessControlFilterHandlerLayer, HeaderModifierFilterHandlerLayer,
    HeaderModifierFilterHandlerLayerError, StaticResponseFilterHandlerLayer,
};
use std::collections::HashMap;
use std::ops::Deref;
use thiserror::Error;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::listener::filter::ListenerFilter;

#[derive(Debug, Clone)]
pub enum ListenerFilterHandlerLayer {
    AccessControl(AccessControlFilterHandlerLayer),
    RequestHeaderModifier(HeaderModifierFilterHandlerLayer),
    ResponseHeaderModifier(HeaderModifierFilterHandlerLayer),
    StaticResponse(StaticResponseFilterHandlerLayer),
}

#[derive(Debug, Error)]
pub enum ListenerFilterHandlerLayerError {
    #[error("AccessControl filter not found")]
    AccessControl,
    #[error("StaticResponse filter not found")]
    StaticResponse,
    #[error("Request header modifier error: {0}")]
    RequestHeaderModifier(#[source] HeaderModifierFilterHandlerLayerError),
    #[error("Response header modifier error: {0}")]
    ResponseHeaderModifier(#[source] HeaderModifierFilterHandlerLayerError),
}

impl
    TryFrom<(
        &HashMap<SharedFilterRef, SharedFilterHandlerLayer>,
        &ListenerFilter,
    )> for ListenerFilterHandlerLayer
{
    type Error = ListenerFilterHandlerLayerError;

    fn try_from(
        (shared_layers, filter): (
            &HashMap<SharedFilterRef, SharedFilterHandlerLayer>,
            &ListenerFilter,
        ),
    ) -> Result<Self, Self::Error> {
        match filter {
            ListenerFilter::AccessControl(filter) => {
                let ref_ = SharedFilterRef::AccessControl(filter.ref_());
                let Some(layer) = shared_layers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_access_control().ok())
                else {
                    return Err(ListenerFilterHandlerLayerError::AccessControl);
                };
                Ok(ListenerFilterHandlerLayer::AccessControl(layer))
            }
            ListenerFilter::RequestHeaderModifier(filter) => {
                let layer = HeaderModifierFilterHandlerLayer::try_from(filter.deref().deref())
                    .map_err(ListenerFilterHandlerLayerError::RequestHeaderModifier)?;
                Ok(ListenerFilterHandlerLayer::RequestHeaderModifier(layer))
            }
            ListenerFilter::ResponseHeaderModifier(filter) => {
                let layer = HeaderModifierFilterHandlerLayer::try_from(filter.deref().deref())
                    .map_err(ListenerFilterHandlerLayerError::ResponseHeaderModifier)?;
                Ok(ListenerFilterHandlerLayer::ResponseHeaderModifier(layer))
            }
            ListenerFilter::StaticResponse(filter) => {
                let ref_ = SharedFilterRef::StaticResponse(filter.ref_());
                let Some(layer) = shared_layers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_static_response().ok())
                else {
                    return Err(ListenerFilterHandlerLayerError::StaticResponse);
                };
                Ok(ListenerFilterHandlerLayer::StaticResponse(layer))
            }
        }
    }
}
