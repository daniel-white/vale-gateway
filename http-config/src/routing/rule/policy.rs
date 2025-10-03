use crate::policy::TimeoutPolicy;
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CopyGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutPolicies {
    #[getset(get_copy = "pub")]
    request: Option<TimeoutPolicy>,

    #[getset(get_copy = "pub")]
    upstream_request: Option<TimeoutPolicy>,
}

impl TimeoutPolicies {
    pub fn is_none(&self) -> bool {
        self.request.is_none() && self.upstream_request.is_none()
    }
}
