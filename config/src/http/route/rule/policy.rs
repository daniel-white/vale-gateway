use crate::http::policy::retry::RetryPolicy;
use crate::http::policy::timeout::TimeoutPolicies;
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CopyGetters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct RulePolicies {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "TimeoutPolicies::is_none")]
    timeouts: TimeoutPolicies,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retries: Option<RetryPolicy>,
}

impl RulePolicies {
    #[must_use] 
    pub fn is_none(&self) -> bool {
        self.timeouts.is_none() && self.retries.is_none()
    }
}
