
use crate::policy::retry::{RetryPolicyHandler, RetryPolicyHandlerConversionError};
use crate::policy::timeout::{TimeoutPolicyHandlers, TimeoutPolicyHandlersConversionError};
use getset::{CloneGetters, Getters};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::listener::policy::ListenerPolicies;
#[derive(Debug, TypedBuilder, Getters, CloneGetters)]
pub struct ListenerPolicyHandlers {
    #[getset(get = "pub")]
    timeouts: TimeoutPolicyHandlers,

    #[getset(get = "pub")]
    retries: Option<RetryPolicyHandler>,
}

#[derive(Debug, Error)]
pub enum ListenerPolicyHandlersConversionError {
    #[error(transparent)]
    Timeouts(#[from] TimeoutPolicyHandlersConversionError),
    #[error(transparent)]
    Retries(#[from] RetryPolicyHandlerConversionError),
}

impl TryFrom<&ListenerPolicies> for ListenerPolicyHandlers {
    type Error = ListenerPolicyHandlersConversionError;

    fn try_from(value: &ListenerPolicies) -> Result<Self, Self::Error> {
        let timeouts = value.timeouts().try_into()?;
        let retries = value
            .retries()
            .as_ref()
            .map(RetryPolicyHandler::try_from)
            .transpose()?;

        let handlers = Self::builder()
            .timeouts(timeouts)
            .retries(retries)
            .build();

        Ok(handlers)
    }
}
