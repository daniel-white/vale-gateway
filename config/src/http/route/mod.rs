use crate::http::route::filter::RouteFilter;
use crate::http::route::host::HostMatcher;
use crate::http::route::rule::Rule;
use derive_more::From;
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub mod filter;
pub mod host;
pub mod rule;

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct RouteRef(String);

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, Getters, CloneGetters, PartialEq)]
pub struct Route {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: RouteRef,

    #[getset(get = "pub")]
    host_matchers: Vec<HostMatcher>,

    #[getset(get = "pub")]
    rules: Vec<Rule>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<RouteFilter>,
}
