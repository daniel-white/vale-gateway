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
