use ipnet::IpNet;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[cfg(feature = "config")]
pub mod config;

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[kube(
    kind = "AccessControlFilter",
    group = "vale-gateway.whitefamily.in",
    version = "v1alpha1",
    namespaced,
    singular = "accesscontrolfilter",
    plural = "accesscontrolfilters"
)]
#[kube(derive = "PartialEq")]
#[kube(status = "AccessControlFilterStatus")]
pub struct AccessControlFilterSpec {
    pub effect: AccessControlFilterEffect,
    pub clients: AccessControlFilterClientMatches,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub enum AccessControlFilterEffect {
    Allow,
    Deny,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlFilterClientMatches {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ips: Vec<IpAddr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "crate::api::v1::schemars::cidr_array")]
    pub ip_ranges: Vec<IpNet>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlFilterStatus {
    /// Conditions describe the current conditions of the `AccessControlFilter`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,

    /// `AttachedRoutes` indicates the number of routes that are using this match
    #[serde(default)]
    pub attached_routes: i32,

    /// `LastUpdated` indicates when the status was last updated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<Time>,
}

/// Condition types for `AccessControlFilter` status
#[derive(Debug, Clone, PartialEq)]
pub enum AccessControlFilterConditionType {
    /// Accepted indicates whether the match configuration is valid and accepted
    Accepted,
    /// Ready indicates whether the match is ready to serve responses
    Ready,
    /// Attached indicates whether the match is attached to any routes
    Attached,
}

impl AccessControlFilterConditionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "Accepted",
            Self::Ready => "Ready",
            Self::Attached => "Attached",
        }
    }
}

/// Condition reasons for `StaticResponseFilter` status
#[derive(Debug, Clone, PartialEq)]
pub enum AccessControlFilterConditionReason {
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

impl AccessControlFilterConditionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "Accepted",
            Self::InvalidConfiguration => "InvalidConfiguration",
            Self::Ready => "Ready",
            Self::NotReady => "NotReady",
            Self::AttachedToRoute => "AttachedToRoute",
            Self::NotAttached => "NotAttached",
        }
    }
}
