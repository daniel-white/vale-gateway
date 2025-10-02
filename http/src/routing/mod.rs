use getset::CopyGetters;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::routing::TimeoutPolicy as TimeoutPolicyConfig;

pub mod matchers;
pub mod upstream;

#[derive(Debug, TypedBuilder, CopyGetters)]
pub struct TimeoutPolicy {
    #[getset(get_copy = "pub")]
    duration: Duration,
}

#[derive(Debug, Error)]
pub enum TimeoutPolicyConversionError {
    #[error("Timeout duration must be greater than zero")]
    InvalidDuration,
}

impl TryFrom<&TimeoutPolicyConfig> for TimeoutPolicy {
    type Error = TimeoutPolicyConversionError;

    fn try_from(value: &TimeoutPolicyConfig) -> Result<Self, Self::Error> {
        if value.duration() <= Duration::ZERO {
            return Err(TimeoutPolicyConversionError::InvalidDuration);
        }

        let policy = Self::builder().duration(value.duration()).build();

        Ok(policy)
    }
}
