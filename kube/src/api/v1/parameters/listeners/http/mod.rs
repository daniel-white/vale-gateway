pub mod filters;

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use filters::GatewayHttpListenerFilters;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHttpListener {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<GatewayHttpListenerFilters>,
}

