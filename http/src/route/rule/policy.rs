use crate::policy::retry::{RetryPolicyHandler, RetryPolicyHandlerConversionError};
use crate::policy::timeout::{TimeoutPolicyConversionError, TimeoutPolicyHandler};
use getset::{CloneGetters, Getters};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::route::rule::policy::RulePolicies as RulePoliciesConfig;
use vg_config::http::route::rule::policy::TimeoutPolicies as TimeoutPoliciesConfig;
use crate::policy::error_response::ErrorResponsePolicyHandler;

#[derive(Debug, TypedBuilder, CloneGetters, Getters)]
pub struct RulePolicies {
    #[getset(get = "pub")]
    timeouts: TimeoutPolicies,

    #[getset(get = "pub")]
    retries: Option<RetryPolicyHandler>,
}

#[derive(Debug, Error)]
pub enum RulePoliciesConversionError {
    #[error("timeout policies are invalid: {0}")]
    TimeoutPolicies(
        #[from]
        #[source]
        TimeoutPoliciesConversionError,
    ),

    #[error("retry policy is invalid: {0}")]
    RetryPolicy(
        #[from]
        #[source]
        RetryPolicyHandlerConversionError,
    ),
}

impl TryFrom<&RulePoliciesConfig> for RulePolicies {
    type Error = RulePoliciesConversionError;

    fn try_from(value: &RulePoliciesConfig) -> Result<Self, Self::Error> {
        let timeouts = value.timeouts().try_into()?;
        let retries = value
            .retries()
            .map(RetryPolicyHandler::try_from)
            .transpose()?;

        let policies = Self::builder().timeouts(timeouts).retries(retries).build();

        Ok(policies)
    }
}

#[derive(Debug, Clone, TypedBuilder, CloneGetters, Getters)]
pub struct TimeoutPolicies {
    #[getset(get_clone = "pub")]
    request: Option<TimeoutPolicyHandler>,

    #[getset(get_clone = "pub")]
    backend_request: Option<TimeoutPolicyHandler>,
}

#[derive(Debug, Error)]
pub enum TimeoutPoliciesConversionError {
    #[error("request timeout policy is invalid: {0}")]
    RequestPolicy(#[source] TimeoutPolicyConversionError),

    #[error("backend request timeout policy is invalid: {0}")]
    BackendRequestPolicy(#[source] TimeoutPolicyConversionError),
}

impl TryFrom<TimeoutPoliciesConfig> for TimeoutPolicies {
    type Error = TimeoutPoliciesConversionError;

    fn try_from(value: TimeoutPoliciesConfig) -> Result<Self, Self::Error> {
        let request = value
            .request()
            .map(|p| {
                p.try_into()
                    .map_err(TimeoutPoliciesConversionError::RequestPolicy)
            })
            .transpose()?;

        let backend_request = value
            .backend_request()
            .map(|p| {
                p.try_into()
                    .map_err(TimeoutPoliciesConversionError::BackendRequestPolicy)
            })
            .transpose()?;

        Ok(Self::builder()
            .request(request)
            .backend_request(backend_request)
            .build())
    }
}
