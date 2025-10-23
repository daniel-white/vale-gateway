use std::collections::HashMap;
use crate::filter::backend_uri_rewriter::{
    BackendUriRewriterFilterHandler, BackendUriRewriterFilterHandlerConversionError,
};
use crate::filter::header_modifier::{
    HeaderModifierFilterHandler, HeaderModifierFilterHandlerConversionError,
};
use crate::filter::redirect_response::{
    RedirectResponseFilterHandler, RedirectResponseFilterHandlerConversionError,
};
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::route::rule::filter::RuleFilter as RuleFilterConfig;
use crate::filter::access_control::AccessControlFilterHandler;
use crate::filter::SharedFilterHandler;
use crate::filter::static_response::StaticResponseFilterHandler;

#[derive(Debug)]
pub enum RuleFilter {
    AccessControl(Arc<AccessControlFilterHandler>),
    RequestHeaderModifier(Arc<HeaderModifierFilterHandler>),
    ResponseHeaderModifier(Arc<HeaderModifierFilterHandler>),
    RedirectResponse(Arc<RedirectResponseFilterHandler>),
    StaticResponse(Arc<StaticResponseFilterHandler>),
    BackendUriRewriter(Arc<BackendUriRewriterFilterHandler>),
}

#[derive(Debug, Error)]
pub enum RuleFilterConversionError {
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

impl TryFrom<(&HashMap<SharedFilterRef, SharedFilterHandler>, &RuleFilterConfig)> for RuleFilter {
    type Error = RuleFilterConversionError;

    fn try_from((shared_filter_handlers, filter): (&HashMap<SharedFilterRef, SharedFilterHandler>, &RuleFilterConfig)) -> Result<Self, Self::Error> {
        match filter {
            RuleFilterConfig::AccessControl(filter) => {
                let ref_ = SharedFilterRef::AccessControl(filter.ref_());
                let Some(filter) = shared_filter_handlers.get(&ref_).and_then(|handler| handler.clone().try_unwrap_access_control().ok()) else {
                    return Err(RuleFilterConversionError::AccessControl)
                };
                Ok(RuleFilter::AccessControl(filter))
            }
            RuleFilterConfig::RequestHeaderModifier(filter) => {
                let handler = HeaderModifierFilterHandler::try_from(filter.deref())
                    .map_err(RuleFilterConversionError::RequestHeaderModifier)?;
                Ok(RuleFilter::RequestHeaderModifier(Arc::new(handler)))
            }
            RuleFilterConfig::ResponseHeaderModifier(filter) => {
                let handler = HeaderModifierFilterHandler::try_from(filter.deref())
                    .map_err(RuleFilterConversionError::ResponseHeaderModifier)?;
                Ok(RuleFilter::ResponseHeaderModifier(Arc::new(handler)))
            }
            RuleFilterConfig::RedirectResponse(filter) => {
                let handler = RedirectResponseFilterHandler::try_from(filter.deref())?;
                Ok(RuleFilter::RedirectResponse(Arc::new(handler)))
            }
            RuleFilterConfig::StaticResponse(filter) => {
                let ref_ = SharedFilterRef::StaticResponse(filter.ref_());
                let Some(filter) = shared_filter_handlers.get(&ref_).and_then(|handler| handler.clone().try_unwrap_static_response().ok()) else {
                    return Err(RuleFilterConversionError::StaticResponse)
                };
                Ok(RuleFilter::StaticResponse(filter))
            }
            RuleFilterConfig::BackendUriRewriter(filter) => {
                let handler = BackendUriRewriterFilterHandler::try_from(filter.deref())?;
                Ok(RuleFilter::BackendUriRewriter(Arc::new(handler)))
            }
        }
    }
}
