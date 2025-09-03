use crate::net::Port;
use getset::{CopyGetters, Getters};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use typed_builder::TypedBuilder;

#[derive(
    Clone,
    Validate,
    CopyGetters,
    Getters,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteBackend {
    #[getset(get_copy = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    weight: Option<u32>,
    #[getset(get_copy = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,
    #[getset(get = "pub")]
    #[builder(setter(into))]
    kind: String,
    #[getset(get = "pub")]
    #[builder(setter(into))]
    name: String,
    #[getset(get = "pub")]
    #[builder(setter(into))]
    namespace: String,
}
