use getset::{CopyGetters, Getters};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use typed_builder::TypedBuilder;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CopyGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    #[getset(get = "pub")]
    #[serde(with = "http_serde_ext::status_code::vec")]
    codes: Vec<StatusCode>,
    #[getset(get_copy = "pub")]
    max_attempts: usize,
    #[getset(get_copy = "pub")]
    backoff: Duration,
}
