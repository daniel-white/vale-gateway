use ipnet::IpNet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[cfg(feature = "config")]
pub mod config;

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum ClientAddressesPolicySource {
    None,
    #[default]
    DirectConnection,
    Header,
    Proxies,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressesPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_header: Option<String>,
    pub source: ClientAddressesPolicySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxies: Option<ClientAddressesPolicyProxies>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ClientAddressesPolicyProxiesTrustedHeaders {
    Forwarded,
    XForwardedFor,
    XForwardedHost,
    XForwardedProto,
    XForwardedBy,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressesPolicyProxies {
    #[serde(default = "trusted_private_ranges_default")]
    pub trust_local_ranges: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_ips: Vec<IpAddr>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "crate::api::v1::schemars::cidr_array")]
    pub trusted_ranges: Vec<IpNet>,

    #[serde(default = "trusted_headers_default", skip_serializing_if = "Vec::is_empty")]
    pub trusted_headers: Vec<ClientAddressesPolicyProxiesTrustedHeaders>,
}

fn trusted_private_ranges_default() -> bool {
    true
}

fn trusted_headers_default() -> Vec<ClientAddressesPolicyProxiesTrustedHeaders> {
    vec![ClientAddressesPolicyProxiesTrustedHeaders::XForwardedFor]
}
