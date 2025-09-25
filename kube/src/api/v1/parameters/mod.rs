use super::deployments::GatewayDeployment;
use crate::api::v1::parameters::listeners::GatewayListener;
use instrumentation::GatewayInstrumentation;
use k8s_openapi::api::core::v1::ServiceSpec;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod instrumentation;
pub mod listeners;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct GatewayCommonParametersSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<GatewayDeployment>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<GatewayConfiguration>,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[kube(
    kind = "GatewayClassParameters",
    group = "vale-gateway.whitefamily.in",
    version = "v1alpha1",
    singular = "gateway-class-parameters",
    plural = "gateway-class-parameters"
)]
#[kube(derive = "PartialEq")]
#[serde(rename_all = "camelCase")]
pub struct GatewayClassParametersSpec {
    #[serde(flatten)]
    pub common: GatewayCommonParametersSpec,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[kube(
    kind = "GatewayParameters",
    group = "vale-gateway.whitefamily.in",
    version = "v1alpha1",
    namespaced,
    singular = "gateway-parameters",
    plural = "gateway-parameters"
)]
#[kube(derive = "PartialEq")]
pub struct GatewayParametersSpec {
    #[serde(flatten)]
    pub common: Option<GatewayCommonParametersSpec>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceSpec>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrumentation: Option<GatewayInstrumentation>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listeners: Option<GatewayListener>,
}
