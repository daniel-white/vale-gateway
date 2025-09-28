use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use typed_builder::TypedBuilder;

pub mod upstream;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CopyGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutPolicy {
    #[getset(get_copy = "pub")]
    duration: Duration,
}
