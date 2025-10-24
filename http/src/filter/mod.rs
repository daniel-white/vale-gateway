use crate::filter::access_control::AccessControlFilterHandler;
use crate::filter::static_response::{
    StaticResponseFilterHandler, StaticResponseFilterHandlerConversionError,
};
use derive_more::TryUnwrap;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;
use vg_config::http::filter::SharedFilter;

pub mod access_control;
pub mod backend_uri_rewriter;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;

#[derive(Debug, Clone, TryUnwrap)]
pub enum SharedFilterHandler {
    AccessControl(Arc<AccessControlFilterHandler>),
    StaticResponse(Arc<StaticResponseFilterHandler>),
}

#[derive(Debug, Error)]
pub enum SharedFilterHandlerConversionError {
    #[error("Invalid static response filter: {0}")]
    StaticResponse(
        #[from]
        #[source]
        StaticResponseFilterHandlerConversionError,
    ),
    // #[error("Invalid access control filter: {0}")]
    // AccessControl(#[from] #[source]AccessControlFilterHandler)
}

impl TryFrom<&SharedFilter> for SharedFilterHandler {
    type Error = SharedFilterHandlerConversionError;
    fn try_from(value: &SharedFilter) -> Result<Self, Self::Error> {
        match value {
            SharedFilter::AccessControl(_) => {
                todo!()
            }
            SharedFilter::StaticResponse(filter) => {
                let handler: StaticResponseFilterHandler = filter.deref().try_into()?;
                Ok(SharedFilterHandler::StaticResponse(Arc::new(handler)))
            }
        }
    }
}
