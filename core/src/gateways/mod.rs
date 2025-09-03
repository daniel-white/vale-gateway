use crate::http::listeners::{HttpListener, HttpListenerBuilder};
use crate::ipc::IpcConfiguration;
use getset::{CloneGetters, CopyGetters, Getters};
use schemars::JsonSchema;
use ::serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::sync::Arc;
use strum::EnumString;
use typed_builder::TypedBuilder;

#[derive(
    Validate,
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    Hash,
    EnumString,
)]
#[serde(rename_all = "lowercase")]
pub enum GatewayVersion {
    #[default]
    #[serde(rename = "v1alpha1")]
    V1Alpha1,
}

#[derive(
    Validate,
    Getters,
    CloneGetters,
    CopyGetters,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    JsonSchema,
    TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Gateway {
    #[getset(get_copy = "pub")]
    #[builder(default)]
    version: GatewayVersion,

    #[getset(get = "pub")]
    ipc: Arc<IpcConfiguration>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    http_listener: Option<Arc<HttpListener>>,
}
