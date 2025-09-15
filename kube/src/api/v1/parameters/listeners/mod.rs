use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use http::GatewayHttpListener;

pub mod http;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayListener {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<GatewayHttpListener>,
}
