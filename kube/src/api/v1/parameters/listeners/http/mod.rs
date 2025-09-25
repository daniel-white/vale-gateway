pub mod filters;

use filters::GatewayHttpListenerFilters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHttpListener {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<GatewayHttpListenerFilters>,
}
