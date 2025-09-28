use getset::{CopyGetters, Getters};
use http::StatusCode;
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::routing::upstream::RetryPolicy as RetryPolicyConfig;

#[derive(Debug, TypedBuilder, CopyGetters, Getters)]
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
    InvalidStatusCode(usize, StatusCode),
    #[error("Max attempts must be at least 1, got {0}")]
    InvalidMaxAttempts(usize),
    #[error("Backoff duration must be greater than zero")]
    InvalidBackoffDuration,
}

impl TryFrom<&RetryPolicyConfig> for RetryPolicy {
    type Error = RetryPolicyConversionError;

    fn try_from(value: &RetryPolicyConfig) -> Result<Self, Self::Error> {
        let codes = value
            .codes()
            .iter()
            .enumerate()
            .map(|(idx, code)| {
                if !code.is_client_error() && !code.is_server_error() {
                    return Err(RetryPolicyConversionError::InvalidStatusCode(idx, *code));
                }
                Ok(*code)
            })
            .collect::<Result<_, _>>()?;

        if value.max_attempts() < 1 {
            return Err(RetryPolicyConversionError::InvalidMaxAttempts(
                value.max_attempts(),
            ));
        }

        if value.backoff() <= Duration::ZERO {
            return Err(RetryPolicyConversionError::InvalidBackoffDuration);
        }

        let policy = Self::builder()
            .codes(codes)
            .max_attempts(value.max_attempts())
            .backoff(value.backoff())
            .build();

        Ok(policy)
    }
}
