use self::filter::RuleFilter;
use self::matcher::RequestMatcher;
use self::policy::TimeoutPolicies;
use crate::policy::RetryPolicy;
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub mod filter;
pub mod matcher;
pub mod policy;

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
pub struct Rule {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[getset(get = "pub")]
    matches: RequestMatcher,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<RuleFilter>,

    #[getset(get_clone = "pub")]
    #[serde(default, skip_serializing_if = "TimeoutPolicies::is_none")]
    timeouts: TimeoutPolicies,

    #[getset(get_clone = "pub")]
    retry: RetryPolicy,
}
