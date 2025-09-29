use ipnet::IpNet;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::CustomResource;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum ClientAddressFilterSource {
    None,
    DirectConnection,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_header: Option<String>,
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
