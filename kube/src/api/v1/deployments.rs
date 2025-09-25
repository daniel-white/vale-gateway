use k8s_openapi::api::apps::v1::DeploymentStrategy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayDeployment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<DeploymentStrategy>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_pull_policy: Option<GatewayDeploymentImagePullPolicy>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<GatewayDeploymentImage>,
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayDeploymentImagePullPolicy {
    Always,
    IfNotPresent,
    Never,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayDeploymentImage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}
