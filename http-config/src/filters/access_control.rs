use derive_more::{From, FromStr};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::net::IpRef;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash, From, FromStr)]
#[serde(transparent)]
pub struct AccessControlFilterRef(String);

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AccessControlEffect {
    Allow,
    Deny,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlFilter {
    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    effect: AccessControlEffect,

    #[getset(get = "pub")]
    clients: Vec<IpRef>,
}
