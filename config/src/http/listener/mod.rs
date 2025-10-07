use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::net::Port;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, CopyGetters)]
#[serde(rename_all = "camelCase")]
pub struct ListenerProtocols {
    #[getset(get_copy = "pub")]
    http: Option<Port>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, Getters)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    protocols: ListenerProtocols,
}