use getset::CopyGetters;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::policy::timeout::TimeoutPolicy;

#[derive(Debug, Clone, TypedBuilder, CopyGetters)]
pub struct TimeoutPolicyHandler {
    #[getset(get_copy = "pub")]
    duration: Duration,
}

#[derive(Debug, Error)]
pub enum TimeoutPolicyConversionError {
    #[error("Timeout duration must be greater than zero")]
    Duration,
}

impl TryFrom<TimeoutPolicy> for TimeoutPolicyHandler {
    type Error = TimeoutPolicyConversionError;

    fn try_from(value: TimeoutPolicy) -> Result<Self, Self::Error> {
        if value.duration() <= Duration::ZERO {
            return Err(TimeoutPolicyConversionError::Duration);
        }

        let policy = Self::builder().duration(value.duration()).build();

        Ok(policy)
    }
}
