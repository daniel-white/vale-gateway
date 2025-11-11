use crate::filter::SharedFilterHandlerLayer;
use crate::handler::{
    AccessControlFilterHandlerLayer, BackendUriRewriterFilterHandlerLayer,
    BackendUriRewriterFilterHandlerLayerError, HeaderModifierFilterHandlerLayer,
    HeaderModifierFilterHandlerLayerError, RedirectResponseFilterHandlerLayer,
    RedirectResponseFilterHandlerLayerError, StaticResponseFilterHandlerLayer,
};
use std::collections::HashMap;
use std::ops::Deref;
use thiserror::Error;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::route::rule::filter::RuleFilter;

#[derive(Debug)]
pub enum RuleFilterHandlerLayer {
    AccessControl(AccessControlFilterHandlerLayer),
    RequestHeaderModifier(HeaderModifierFilterHandlerLayer),
    ResponseHeaderModifier(HeaderModifierFilterHandlerLayer),
    RedirectResponse(RedirectResponseFilterHandlerLayer),
    StaticResponse(StaticResponseFilterHandlerLayer),
    BackendUriRewriter(BackendUriRewriterFilterHandlerLayer),
}

#[derive(Debug, Error)]
pub enum RuleFilterHandlerLayerError {
    #[error("AccessControl filter not found")]
    AccessControl,
    #[error("StaticResponse filter not found")]
    StaticResponse,
    #[error("Request header modifier error: {0}")]
    RequestHeaderModifier(#[source] HeaderModifierFilterHandlerLayerError),
    #[error("Response header modifier error: {0}")]
    ResponseHeaderModifier(#[source] HeaderModifierFilterHandlerLayerError),
    #[error(transparent)]
    RedirectResponse(#[from] RedirectResponseFilterHandlerLayerError),
    #[error(transparent)]
    BackendUriRewriter(#[from] BackendUriRewriterFilterHandlerLayerError),
}

impl
    TryFrom<(
        &HashMap<SharedFilterRef, SharedFilterHandlerLayer>,
        &RuleFilter,
    )> for RuleFilterHandlerLayer
{
    type Error = RuleFilterHandlerLayerError;

    fn try_from(
        (shared_layers, filter): (
            &HashMap<SharedFilterRef, SharedFilterHandlerLayer>,
            &RuleFilter,
        ),
    ) -> Result<Self, Self::Error> {
        match filter {
            RuleFilter::AccessControl(filter) => {
                let ref_ = SharedFilterRef::AccessControl(filter.ref_());
                let Some(layers) = shared_layers
                    .get(&ref_)
                    .and_then(|layer| layer.clone().try_unwrap_access_control().ok())
                else {
                    return Err(RuleFilterHandlerLayerError::AccessControl);
                };
                Ok(Self::AccessControl(layers))
            }
            RuleFilter::RequestHeaderModifier(filter) => {
                let layer = HeaderModifierFilterHandlerLayer::try_from(filter.deref().deref())
                    .map_err(RuleFilterHandlerLayerError::RequestHeaderModifier)?;
                Ok(Self::RequestHeaderModifier(layer))
            }
            RuleFilter::ResponseHeaderModifier(filter) => {
                let layer = HeaderModifierFilterHandlerLayer::try_from(filter.deref().deref())
                    .map_err(RuleFilterHandlerLayerError::ResponseHeaderModifier)?;
                Ok(Self::ResponseHeaderModifier(layer))
            }
            RuleFilter::RedirectResponse(filter) => {
                let handler: RedirectResponseFilterHandlerLayer = filter.deref().try_into()?;
                Ok(Self::RedirectResponse(handler))
            }
            RuleFilter::StaticResponse(filter) => {
                let ref_ = SharedFilterRef::StaticResponse(filter.ref_());
                let Some(layer) = shared_layers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_static_response().ok())
                else {
                    return Err(RuleFilterHandlerLayerError::StaticResponse);
                };
                Ok(Self::StaticResponse(layer))
            }
            RuleFilter::BackendUriRewriter(filter) => {
                let layer: BackendUriRewriterFilterHandlerLayer = filter.deref().try_into()?;
                Ok(Self::BackendUriRewriter(layer))
            }
        }
    }
}
