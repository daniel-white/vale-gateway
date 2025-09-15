use serde::{Deserialize, Serialize};
use k8s_openapi::api::core::v1::ServiceSpec;
use schemars::JsonSchema;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct GatewayServiceSpec {
    #[serde(flatten)]
    pub spec: ServiceSpec,
}