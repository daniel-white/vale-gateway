use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use serde_with::DurationSecondsWithFrac;
use serde_with::serde_as;
use std::time::Duration;
use typed_builder::TypedBuilder;

#[serde_as]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, CopyGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutPolicy {
    #[getset(get_copy = "pub")]
    #[serde(rename = "durationSecs")]
    #[serde_as(as = "DurationSecondsWithFrac")]
    duration: Duration,
}

#[derive(
    Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CopyGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutPolicies {
    #[getset(get_copy = "pub")]
    request: Option<TimeoutPolicy>,

    #[getset(get_copy = "pub")]
    backend_request: Option<TimeoutPolicy>,
}

impl TimeoutPolicies {
    pub fn is_none(&self) -> bool {
        self.request.is_none() && self.backend_request.is_none()
    }
}
