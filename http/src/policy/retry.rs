use getset::{CopyGetters, Getters};
use http::StatusCode;
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::policy::retry::RetryPolicy;

#[derive(Debug, Clone, TypedBuilder, CopyGetters, Getters)]
pub struct RetryPolicyHandler {
    #[getset(get = "pub")]
    codes: HashSet<StatusCode>,
    #[getset(get_copy = "pub")]
    max_attempts: usize,
    #[getset(get_copy = "pub")]
    backoff: Duration,
}

impl TryFrom<&RetryPolicy> for RetryPolicyHandler {
    type Error = RetryPolicyHandlerConversionError;

    fn try_from(value: &RetryPolicy) -> Result<Self, Self::Error> {
        let codes = value
            .codes()
            .iter()
            .enumerate()
            .map(|(idx, code)| {
                if !code.is_client_error() && !code.is_server_error() {
                    return Err(RetryPolicyHandlerConversionError::StatusCode(idx, *code));
                }
                Ok(*code)
            })
            .collect::<Result<_, _>>()?;

        if value.max_attempts() < 1 {
            return Err(RetryPolicyHandlerConversionError::MaxAttempts(value.max_attempts()));
        }

        if value.backoff() <= Duration::ZERO {
            return Err(RetryPolicyHandlerConversionError::BackoffDuration);
        }

        let policy = Self::builder()
            .codes(codes)
            .max_attempts(value.max_attempts())
            .backoff(value.backoff())
            .build();

        Ok(policy)
    }
}

impl RetryPolicyHandler {
    #[must_use] 
    pub fn should_retry(&self, code: StatusCode, current_attempt: usize) -> bool {
        self.codes.contains(&code) && current_attempt < self.max_attempts
    }
}

#[derive(Debug, Error)]
pub enum RetryPolicyHandlerConversionError {
    #[error("Invalid status code ({1}) at index {0}")]
    StatusCode(usize, StatusCode),
    #[error("Max attempts must be at least 1, got {0}")]
    MaxAttempts(usize),
    #[error("Backoff duration must be greater than zero")]
    BackoffDuration,
}
