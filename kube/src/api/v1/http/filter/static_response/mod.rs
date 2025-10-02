use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "config")]
pub mod config;

#[derive(Default, CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[kube(
    kind = "StaticResponseFilter",
    group = "vale-gateway.whitefamily.in",
    version = "v1alpha1",
    namespaced,
    singular = "staticresponsefilter",
    plural = "staticresponsefilters"
)]
#[kube(derive = "Default")]
#[kube(derive = "PartialEq")]
#[kube(status = "StaticResponseFilterStatus")]
pub struct StaticResponseFilterSpec {
    pub status_code: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<StaticResponseFilterBody>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub enum StaticResponseFilterBodyFormat {
    Text,
    Binary,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct StaticResponseFilterBody {
    pub content_type: String,
    pub format: StaticResponseFilterBodyFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "crate::api::v1::schemars::base64_string")]
    pub binary: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseFilterStatus {
    /// Conditions describe the current conditions of the `StaticResponseFilter`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,

    /// `AttachedRoutes` indicates the number of routes that are using this match
    #[serde(default)]
    pub attached_routes: i32,

    /// `LastUpdated` indicates when the status was last updated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<Time>,
}

/// Condition types for `StaticResponseFilter` status
#[derive(Debug, Clone, PartialEq)]
pub enum StaticResponseFilterConditionType {
    /// Accepted indicates whether the match configuration is valid and accepted
    Accepted,
    /// Ready indicates whether the match is ready to serve responses
    Ready,
    /// Attached indicates whether the match is attached to any routes
    Attached,
}

impl StaticResponseFilterConditionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StaticResponseFilterConditionType::Accepted => "Accepted",
            StaticResponseFilterConditionType::Ready => "Ready",
            StaticResponseFilterConditionType::Attached => "Attached",
        }
    }
}

/// Condition reasons for `StaticResponseFilter` status
#[derive(Debug, Clone, PartialEq)]
pub enum StaticResponseFilterConditionReason {
    /// Accepted - The match configuration is valid
    Accepted,
    /// `InvalidConfiguration` - The match configuration is invalid
    InvalidConfiguration,
    /// Ready - The match is ready to serve responses
    Ready,
    /// `NotReady` - The match is not ready to serve responses
    NotReady,
    /// `AttachedToRoute` - The match is attached to one or more routes
    AttachedToRoute,
    /// `NotAttached` - The match is not attached to any routes
    NotAttached,
}

impl StaticResponseFilterConditionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            StaticResponseFilterConditionReason::Accepted => "Accepted",
            StaticResponseFilterConditionReason::InvalidConfiguration => "InvalidConfiguration",
            StaticResponseFilterConditionReason::Ready => "Ready",
            StaticResponseFilterConditionReason::NotReady => "NotReady",
            StaticResponseFilterConditionReason::AttachedToRoute => "AttachedToRoute",
            StaticResponseFilterConditionReason::NotAttached => "NotAttached",
        }
    }
}
