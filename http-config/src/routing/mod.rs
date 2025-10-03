use crate::filter::RouteRuleFilter;
use crate::policy::{RetryPolicy, TimeoutPolicy};
use crate::routing::matchers::RequestMatcher;
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub mod matchers;
pub mod upstream;

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

#[derive(
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
pub struct RouteRule {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[getset(get = "pub")]
    matches: RequestMatcher,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<RouteRuleFilter>,

    #[getset(get_clone = "pub")]
    #[serde(default, skip_serializing_if = "TimeoutPolicies::is_none")]
    timeouts: TimeoutPolicies,

    #[getset(get_clone = "pub")]
    retry: RetryPolicy,
}
