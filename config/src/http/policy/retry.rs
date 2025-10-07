use getset::{CopyGetters, Getters};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_with::DurationSecondsWithFrac;
use serde_with::serde_as;
use std::time::Duration;
use typed_builder::TypedBuilder;

#[serde_as]
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
    #[serde(rename = "backoffSecs")]
    #[serde_as(as = "DurationSecondsWithFrac")]
    backoff: Duration,
}
