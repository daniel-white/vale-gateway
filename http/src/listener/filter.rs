use crate::filter::SharedFilterHandler;
use crate::filter::access_control::AccessControlFilterHandler;
use crate::filter::header_modifier::{
    HeaderModifierFilterHandler, HeaderModifierFilterHandlerConversionError,
};
use crate::filter::static_response::StaticResponseFilterHandler;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::listener::filter::ListenerFilter;

#[derive(Debug)]
pub enum ListenerFilterHandler {
    AccessControl(Arc<AccessControlFilterHandler>),
    RequestHeaderModifier(Arc<HeaderModifierFilterHandler>),
    ResponseHeaderModifier(Arc<HeaderModifierFilterHandler>),
    StaticResponse(Arc<StaticResponseFilterHandler>),
}

#[derive(Debug, Error)]
pub enum ListenerFilterHandlerConversionError {
    #[error("AccessControl filter not found")]
    AccessControl,
    #[error("StaticResponse filter not found")]
    StaticResponse,
    #[error("Request header modifier error: {0}")]
    RequestHeaderModifier(#[source] HeaderModifierFilterHandlerConversionError),
    #[error("Response header modifier error: {0}")]
    ResponseHeaderModifier(#[source] HeaderModifierFilterHandlerConversionError),
}

impl
    TryFrom<(
        &HashMap<SharedFilterRef, SharedFilterHandler>,
        &ListenerFilter,
    )> for ListenerFilterHandler
{
    type Error = ListenerFilterHandlerConversionError;

    fn try_from(
        (shared_filter_handlers, filter): (
            &HashMap<SharedFilterRef, SharedFilterHandler>,
            &ListenerFilter,
        ),
    ) -> Result<Self, Self::Error> {
        match filter {
            ListenerFilter::AccessControl(filter) => {
                let ref_ = SharedFilterRef::AccessControl(filter.ref_());
                let Some(filter) = shared_filter_handlers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_access_control().ok())
                else {
                    return Err(ListenerFilterHandlerConversionError::AccessControl);
                };
                Ok(ListenerFilterHandler::AccessControl(filter))
            }
            ListenerFilter::RequestHeaderModifier(filter) => {
                let handler = HeaderModifierFilterHandler::try_from(filter.deref())
                    .map_err(ListenerFilterHandlerConversionError::RequestHeaderModifier)?;
                Ok(ListenerFilterHandler::RequestHeaderModifier(Arc::new(
                    handler,
                )))
            }
            ListenerFilter::ResponseHeaderModifier(filter) => {
                let handler = HeaderModifierFilterHandler::try_from(filter.deref())
                    .map_err(ListenerFilterHandlerConversionError::ResponseHeaderModifier)?;
                Ok(ListenerFilterHandler::ResponseHeaderModifier(Arc::new(
                    handler,
                )))
            }
            ListenerFilter::StaticResponse(filter) => {
                let ref_ = SharedFilterRef::StaticResponse(filter.ref_());
                let Some(filter) = shared_filter_handlers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_static_response().ok())
                else {
                    return Err(ListenerFilterHandlerConversionError::StaticResponse);
                };
                Ok(ListenerFilterHandler::StaticResponse(filter))
            }
        }
    }
}
