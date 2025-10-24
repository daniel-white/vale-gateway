use crate::filter::SharedFilterHandler;
use crate::filter::access_control::AccessControlFilterHandler;
use crate::filter::backend_uri_rewriter::{
    BackendUriRewriterFilterHandler, BackendUriRewriterFilterHandlerConversionError,
};
use crate::filter::header_modifier::{
    HeaderModifierFilterHandler, HeaderModifierFilterHandlerConversionError,
};
use crate::filter::redirect_response::{
    RedirectResponseFilterHandler, RedirectResponseFilterHandlerConversionError,
};
use crate::filter::static_response::StaticResponseFilterHandler;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::route::rule::filter::RuleFilter;

#[derive(Debug)]
pub enum RuleFilterHandler {
    AccessControl(Arc<AccessControlFilterHandler>),
    RequestHeaderModifier(Arc<HeaderModifierFilterHandler>),
    ResponseHeaderModifier(Arc<HeaderModifierFilterHandler>),
    RedirectResponse(Arc<RedirectResponseFilterHandler>),
    StaticResponse(Arc<StaticResponseFilterHandler>),
    BackendUriRewriter(Arc<BackendUriRewriterFilterHandler>),
}

#[derive(Debug, Error)]
pub enum RuleFilterHandlerConversionError {
    #[error("AccessControl filter not found")]
    AccessControl,
    #[error("StaticResponse filter not found")]
    StaticResponse,
    #[error("Request header modifier error: {0}")]
    RequestHeaderModifier(#[source] HeaderModifierFilterHandlerConversionError),
    #[error("Response header modifier error: {0}")]
    ResponseHeaderModifier(#[source] HeaderModifierFilterHandlerConversionError),
    #[error("Redirect response error: {0}")]
    RedirectResponse(
        #[from]
        #[source]
        RedirectResponseFilterHandlerConversionError,
    ),
    #[error("Backend URI rewriter error: {0}")]
    BackendUriRewriter(
        #[from]
        #[source]
        BackendUriRewriterFilterHandlerConversionError,
    ),
}

impl TryFrom<(&HashMap<SharedFilterRef, SharedFilterHandler>, &RuleFilter)> for RuleFilterHandler {
    type Error = RuleFilterHandlerConversionError;

    fn try_from(
        (shared_filter_handlers, filter): (
            &HashMap<SharedFilterRef, SharedFilterHandler>,
            &RuleFilter,
        ),
    ) -> Result<Self, Self::Error> {
        match filter {
            RuleFilter::AccessControl(filter) => {
                let ref_ = SharedFilterRef::AccessControl(filter.ref_());
                let Some(filter) = shared_filter_handlers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_access_control().ok())
                else {
                    return Err(RuleFilterHandlerConversionError::AccessControl);
                };
                Ok(RuleFilterHandler::AccessControl(filter))
            }
            RuleFilter::RequestHeaderModifier(filter) => {
                let handler = HeaderModifierFilterHandler::try_from(filter.deref())
                    .map_err(RuleFilterHandlerConversionError::RequestHeaderModifier)?;
                Ok(RuleFilterHandler::RequestHeaderModifier(Arc::new(handler)))
            }
            RuleFilter::ResponseHeaderModifier(filter) => {
                let handler = HeaderModifierFilterHandler::try_from(filter.deref())
                    .map_err(RuleFilterHandlerConversionError::ResponseHeaderModifier)?;
                Ok(RuleFilterHandler::ResponseHeaderModifier(Arc::new(handler)))
            }
            RuleFilter::RedirectResponse(filter) => {
                let handler = RedirectResponseFilterHandler::try_from(filter.deref())?;
                Ok(RuleFilterHandler::RedirectResponse(Arc::new(handler)))
            }
            RuleFilter::StaticResponse(filter) => {
                let ref_ = SharedFilterRef::StaticResponse(filter.ref_());
                let Some(filter) = shared_filter_handlers
                    .get(&ref_)
                    .and_then(|handler| handler.clone().try_unwrap_static_response().ok())
                else {
                    return Err(RuleFilterHandlerConversionError::StaticResponse);
                };
                Ok(RuleFilterHandler::StaticResponse(filter))
            }
            RuleFilter::BackendUriRewriter(filter) => {
                let handler = BackendUriRewriterFilterHandler::try_from(filter.deref())?;
                Ok(RuleFilterHandler::BackendUriRewriter(Arc::new(handler)))
            }
        }
    }
}
