use getset::Getters;
use hickory_proto::rr::Name;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Getters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpUpstreamUriRewriteFilter {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(setter(into))]
    #[schemars(schema_with = "crate::schemars::dns_name")]
    host: Option<Name>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(into))]
    path: Option<HttpUpstreamUriPathRewrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum HttpUpstreamUriPathRewrite {
    Full(String),
    PrefixMatch(String),
}
