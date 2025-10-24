use crate::policy::retry::{RetryPolicyHandler, RetryPolicyHandlerConversionError};
use crate::policy::timeout::{TimeoutPolicyHandlers, TimeoutPolicyHandlersConversionError};
use getset::{CloneGetters, Getters};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::route::rule::policy::RulePolicies;

#[derive(Debug, TypedBuilder, CloneGetters, Getters)]
pub struct RulePolicyHandlers {
    #[getset(get = "pub")]
    timeouts: TimeoutPolicyHandlers,

    #[getset(get = "pub")]
    retries: Option<RetryPolicyHandler>,
}

#[derive(Debug, Error)]
pub enum RulePoliciesConversionError {
    #[error("timeout policies are invalid: {0}")]
    TimeoutPolicies(
        #[from]
        #[source]
        TimeoutPolicyHandlersConversionError,
    ),

    #[error("retry policy is invalid: {0}")]
    RetryPolicy(
        #[from]
        #[source]
        RetryPolicyHandlerConversionError,
    ),
}

impl TryFrom<&RulePolicies> for RulePolicyHandlers {
    type Error = RulePoliciesConversionError;

    fn try_from(value: &RulePolicies) -> Result<Self, Self::Error> {
        let timeouts = value.timeouts().try_into()?;
        let retries = value
            .retries()
            .as_ref()
            .map(RetryPolicyHandler::try_from)
            .transpose()?;

        let policies = Self::builder().timeouts(timeouts).retries(retries).build();

        Ok(policies)
    }
}
