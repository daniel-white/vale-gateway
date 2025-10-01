use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "config")]
pub mod config;

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum ErrorResponseFilterKind {
    #[default]
    Empty,
    Html,
    ProblemDetail,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    kind = "ErrorResponseFilter",
    group = "vale-gateway.whitefamily.in",
    version = "v1alpha1",
    namespaced,
    singular = "errorresponsefilter",
    plural = "errorresponsefilters"
)]
#[kube(derive = "PartialEq")]
#[kube(status = "ErrorResponseFilterStatus")]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseFilterSpec {
    pub kind: ErrorResponseFilterKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem_detail: Option<ProblemDetailErrorResponse>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailErrorResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseFilterStatus {
    /// Conditions describe the current conditions of the `ErrorResponseFilter`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,

    /// `AttachedRoutes` indicates the number of routes that are using this filter
    #[serde(default)]
    pub attached_routes: i32,

    /// `LastUpdated` indicates when the status was last updated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<Time>,
}
