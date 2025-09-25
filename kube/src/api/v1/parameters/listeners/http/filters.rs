use crate::api::v1::common::Ref;
use gateway_api::common::{HeaderModifier, RequestRedirect};
use ipnet::IpNet;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::CustomResource;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHttpListenerFilters {
    #[serde(rename = "type")]
    pub r#type: GatewayHttpListenerFilterType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header_modifier: Option<HeaderModifier>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_header_modifier: Option<HeaderModifier>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_redirect: Option<RequestRedirect>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_control: Option<Ref>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_address: Option<Ref>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_response: Option<Ref>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayHttpListenerFilterType {
    RequestHeaderModifier,
    ResponseHeaderModifier,
    RequestRedirect,
    ClientAddress,
    AccessControl,
    ErrorResponse,
}

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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum ClientAddressFilterSource {
    Header,
    Proxies,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    kind = "ClientAddressFilter",
    group = "vale-gateway.whitefamily.in",
    version = "v1alpha1",
    namespaced,
    singular = "clientaddressfilter",
    plural = "clientaddressfilters"
)]
#[kube(derive = "PartialEq")]
#[kube(status = "ClientAddressFilterStatus")]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressFilterSpec {
    pub source: ClientAddressFilterSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxies: Option<ClientAddressFilterProxies>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressFilterStatus {
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

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ClientAddressFilterProxiesTrustedHeaders {
    Forwarded,
    XForwardedFor,
    XForwardedHost,
    XForwardedProto,
    XForwardedBy,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressFilterProxies {
    #[serde(default = "trusted_private_ranges_default")]
    pub trust_local_ranges: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_ips: Vec<IpAddr>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "cidr_array_schema")]
    pub trusted_ranges: Vec<IpNet>,

    #[serde(
        default = "trusted_headers_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub trusted_headers: Vec<ClientAddressFilterProxiesTrustedHeaders>,
}

fn trusted_private_ranges_default() -> bool {
    true
}

fn trusted_headers_default() -> Vec<ClientAddressFilterProxiesTrustedHeaders> {
    vec![ClientAddressFilterProxiesTrustedHeaders::XForwardedFor]
}

pub fn cidr_array_schema(_: &mut SchemaGenerator) -> Schema {
    // Create schema for a single CIDR
    let item_schema = json_schema!({
        "type": "string",
        "format": "cidr",
    });

    // Create schema for array of CIDRs
    json_schema!({
        "type": "array",
        "items": item_schema,
        "uniqueItems": true,
    })
}

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
    pub format: StaticResponseFilterBodyFormat,
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseFilterStatus {
    /// Conditions describe the current conditions of the `StaticResponseFilter`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,

    /// `AttachedRoutes` indicates the number of routes that are using this filter
    #[serde(default)]
    pub attached_routes: i32,

    /// `LastUpdated` indicates when the status was last updated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<Time>,
}

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
    #[schemars(schema_with = "cidr_array_schema")]
    pub ip_ranges: Vec<IpNet>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlFilterStatus {
    /// Conditions describe the current conditions of the `AccessControlFilter`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,

    /// `AttachedRoutes` indicates the number of routes that are using this filter
    #[serde(default)]
    pub attached_routes: i32,

    /// `LastUpdated` indicates when the status was last updated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<Time>,
}

/// Condition types for `StaticResponseFilter` status
#[derive(Debug, Clone, PartialEq)]
pub enum StaticResponseFilterConditionType {
    /// Accepted indicates whether the filter configuration is valid and accepted
    Accepted,
    /// Ready indicates whether the filter is ready to serve responses
    Ready,
    /// Attached indicates whether the filter is attached to any routes
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
    /// Accepted - The filter configuration is valid
    Accepted,
    /// `InvalidConfiguration` - The filter configuration is invalid
    InvalidConfiguration,
    /// Ready - The filter is ready to serve responses
    Ready,
    /// `NotReady` - The filter is not ready to serve responses
    NotReady,
    /// `AttachedToRoute` - The filter is attached to one or more routes
    AttachedToRoute,
    /// `NotAttached` - The filter is not attached to any routes
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

/// Condition types for `AccessControlFilter` status
#[derive(Debug, Clone, PartialEq)]
pub enum AccessControlFilterConditionType {
    /// Accepted indicates whether the filter configuration is valid and accepted
    Accepted,
    /// Ready indicates whether the filter is ready to serve responses
    Ready,
    /// Attached indicates whether the filter is attached to any routes
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
    /// Accepted - The filter configuration is valid
    Accepted,
    /// `InvalidConfiguration` - The filter configuration is invalid
    InvalidConfiguration,
    /// Ready - The filter is ready to serve responses
    Ready,
    /// `NotReady` - The filter is not ready to serve responses
    NotReady,
    /// `AttachedToRoute` - The filter is attached to one or more routes
    AttachedToRoute,
    /// `NotAttached` - The filter is not attached to any routes
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
