use getset::CloneGetters;
use http::Method;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, CloneGetters, TypedBuilder)]
pub struct MethodMatcher {
    #[getset(get_clone = "pub")]
    #[serde(with = "http_serde::method")]
    method: Method,
}
