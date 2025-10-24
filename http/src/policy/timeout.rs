use getset::{CloneGetters, CopyGetters, Getters};
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::policy::timeout::{TimeoutPolicies, TimeoutPolicy};

#[derive(Debug, Clone, TypedBuilder, CopyGetters)]
pub struct TimeoutPolicyHandler {
    #[getset(get_copy = "pub")]
    duration: Duration,
}

#[derive(Debug, Error)]
pub enum TimeoutPolicyHandlerConversionError {
    #[error("Timeout duration must be greater than zero")]
    Duration,
}

impl TryFrom<TimeoutPolicy> for TimeoutPolicyHandler {
    type Error = TimeoutPolicyHandlerConversionError;

    fn try_from(value: TimeoutPolicy) -> Result<Self, Self::Error> {
        if value.duration() <= Duration::ZERO {
            return Err(TimeoutPolicyHandlerConversionError::Duration);
        }

        let policy = Self::builder().duration(value.duration()).build();

        Ok(policy)
    }
}

#[derive(Debug, Clone, TypedBuilder, CloneGetters, Getters)]
pub struct TimeoutPolicyHandlers {
    #[getset(get_clone = "pub")]
    request: Option<TimeoutPolicyHandler>,

    #[getset(get_clone = "pub")]
    backend_request: Option<TimeoutPolicyHandler>,
}

#[derive(Debug, Error)]
pub enum TimeoutPolicyHandlersConversionError {
    #[error("request timeout policy is invalid: {0}")]
    RequestPolicy(#[source] TimeoutPolicyHandlerConversionError),

    #[error("backend request timeout policy is invalid: {0}")]
    BackendRequestPolicy(#[source] TimeoutPolicyHandlerConversionError),
}

impl TryFrom<&TimeoutPolicies> for TimeoutPolicyHandlers {
    type Error = TimeoutPolicyHandlersConversionError;

    fn try_from(value: &TimeoutPolicies) -> Result<Self, Self::Error> {
        let request = value
            .request()
            .map(|p| {
                p.try_into()
                    .map_err(TimeoutPolicyHandlersConversionError::RequestPolicy)
            })
            .transpose()?;

        let backend_request = value
            .backend_request()
            .map(|p| {
                p.try_into()
                    .map_err(TimeoutPolicyHandlersConversionError::BackendRequestPolicy)
            })
            .transpose()?;

        Ok(Self::builder()
            .request(request)
            .backend_request(backend_request)
            .build())
    }
}
