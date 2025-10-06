use crate::routing::route::host::HostMatcher;
use crate::routing::rule::Rule;
use getset::Getters;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub mod host;

#[derive(Debug, Serialize, Deserialize, TypedBuilder, Getters)]
pub struct Route {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    host_matchers: Vec<HostMatcher>,

    #[getset(get = "pub")]
    rules: Vec<Rule>,
}
