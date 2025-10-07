use crate::http::route::host::HostMatcher;
use derive_more::From;
use getset::Getters;
use rule::Rule;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub mod host;
pub mod rule;

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct RouteRef(String);

#[derive(Debug, Clone, Serialize, Deserialize, TypedBuilder, Getters)]
pub struct Route {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: RouteRef,

    #[getset(get = "pub")]
    host_matchers: Vec<HostMatcher>,

    #[getset(get = "pub")]
    rules: Vec<Rule>,
}
