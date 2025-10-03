use getset::{CopyGetters, Getters};
use http::StatusCode;
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::policy::RetryPolicy as RetryPolicyConfig;
use vg_http_config::policy::TimeoutPolicy as TimeoutPolicyConfig;

#[derive(Debug, Clone, TypedBuilder, CopyGetters, Getters)]
pub struct RetryPolicy {
    #[getset(get = "pub")]
    codes: HashSet<StatusCode>,
    #[getset(get_copy = "pub")]
    max_attempts: usize,
    #[getset(get_copy = "pub")]
    backoff: Duration,
}

impl RetryPolicy {
    pub fn should_retry(&self, code: StatusCode, current_attempt: usize) -> bool {
        self.codes.contains(&code) && current_attempt < self.max_attempts
    }
}

#[derive(Debug, Error)]
pub enum RetryPolicyConversionError {
    #[error("Invalid status code ({1}) at index {0}")]
    StatusCode(usize, StatusCode),
    #[error("Max attempts must be at least 1, got {0}")]
    MaxAttempts(usize),
    #[error("Backoff duration must be greater than zero")]
    BackoffDuration,
}

impl TryFrom<RetryPolicyConfig> for RetryPolicy {
    type Error = RetryPolicyConversionError;

    fn try_from(value: RetryPolicyConfig) -> Result<Self, Self::Error> {
        let codes = value
            .codes()
            .iter()
            .enumerate()
            .map(|(idx, code)| {
                if !code.is_client_error() && !code.is_server_error() {
                    return Err(RetryPolicyConversionError::StatusCode(idx, *code));
                }
                Ok(*code)
            })
            .collect::<Result<_, _>>()?;

        if value.max_attempts() < 1 {
            return Err(RetryPolicyConversionError::MaxAttempts(
                value.max_attempts(),
            ));
        }

        if value.backoff() <= Duration::ZERO {
            return Err(RetryPolicyConversionError::BackoffDuration);
        }

        let policy = Self::builder()
            .codes(codes)
            .max_attempts(value.max_attempts())
            .backoff(value.backoff())
            .build();

        Ok(policy)
    }
}

#[derive(Debug, Clone, TypedBuilder, CopyGetters)]
pub struct TimeoutPolicy {
    #[getset(get_copy = "pub")]
    duration: Duration,
}

#[derive(Debug, Error)]
pub enum TimeoutPolicyConversionError {
    #[error("Timeout duration must be greater than zero")]
    Duration,
}

impl TryFrom<TimeoutPolicyConfig> for TimeoutPolicy {
    type Error = TimeoutPolicyConversionError;

    fn try_from(value: TimeoutPolicyConfig) -> Result<Self, Self::Error> {
        if value.duration() <= Duration::ZERO {
            return Err(TimeoutPolicyConversionError::Duration);
        }

        let policy = Self::builder().duration(value.duration()).build();

        Ok(policy)
    }
}
