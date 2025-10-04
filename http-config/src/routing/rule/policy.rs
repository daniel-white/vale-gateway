use crate::policy::{RetryPolicy, TimeoutPolicy};
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Getters,
    CopyGetters,
    CloneGetters,
    TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct RulePolicies {
    #[getset(get_clone = "pub")]
    #[serde(default, skip_serializing_if = "TimeoutPolicies::is_none")]
    timeouts: TimeoutPolicies,

    #[getset(get_clone = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retries: Option<RetryPolicy>,
}

impl RulePolicies {
    pub fn is_none(&self) -> bool {
        self.timeouts.is_none() && self.retries.is_none()
    }
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
