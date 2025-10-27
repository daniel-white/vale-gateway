use crate::http::backend::BackendRef;
use crate::http::route::rule::filter::RuleBackendFilter;
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::net::Port;

#[derive(
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    TypedBuilder,
    Getters,
    CloneGetters,
    CopyGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct RuleBackend {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    ref_: BackendRef,

    #[getset(get_copy = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,

    #[getset(get_copy = "pub")]
    weight: u32,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<RuleBackendFilter>,
}

/// WeightedBackendRef represents a backend reference with an associated weight for load balancing
#[derive(
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Clone,
    TypedBuilder,
    Getters,
    CloneGetters,
    CopyGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct WeightedBackendRef {
    #[getset(get_clone = "pub")]
    backend_ref: BackendRef,

    #[getset(get_copy = "pub")]
    weight: u32,
}
