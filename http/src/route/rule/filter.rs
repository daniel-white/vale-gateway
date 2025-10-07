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
use vg_config::http::filter::access_control::AccessControlFilterRef;
use vg_config::http::filter::error_response::ErrorResponseFilterRef;
use vg_config::http::filter::static_response::StaticResponseFilterRef;
use vg_config::http::route::rule::filter::RuleFilter as RuleFilterConfig;

#[derive(Debug)]
pub enum RuleFilter {
    AccessControl(Arc<AccessControlFilterRef>),
    ErrorResponse(Arc<ErrorResponseFilterRef>),
    RequestHeaderModifier(Arc<HeaderModifierFilterHandler>),
    ResponseHeaderModifier(Arc<HeaderModifierFilterHandler>),
    RedirectResponse(Arc<RedirectResponseFilterHandler>),
    StaticResponse(Arc<StaticResponseFilterRef>),
    BackendUriRewriter(Arc<BackendUriRewriterFilterHandler>),
}

#[derive(Debug, Error)]
pub enum RuleFilterConversionError {
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

impl TryFrom<&RuleFilterConfig> for RuleFilter {
    type Error = RuleFilterConversionError;

    fn try_from(value: &RuleFilterConfig) -> Result<Self, Self::Error> {
        match value {
            RuleFilterConfig::AccessControl(filter) => {
                Ok(RuleFilter::AccessControl(Arc::new(filter.ref_())))
            }
            RuleFilterConfig::ErrorResponse(filter) => {
                Ok(RuleFilter::ErrorResponse(Arc::new(filter.ref_())))
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
                Ok(RuleFilter::StaticResponse(Arc::new(filter.ref_())))
            }
            RuleFilterConfig::BackendUriRewriter(filter) => {
                let handler = BackendUriRewriterFilterHandler::try_from(filter.deref())?;
                Ok(RuleFilter::BackendUriRewriter(Arc::new(handler)))
            }
        }
    }
}
