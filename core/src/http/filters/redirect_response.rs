use crate::net::Port;
use getset::Getters;
use hickory_proto::rr::Name;
use http::uri::Scheme;
use http::StatusCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Getters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpRedirectResponseFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    kind: HttpRedirectResponseKind,

    #[getset(get = "pub")]
    #[serde(
        with = "http_serde_ext::scheme::option",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[builder(setter(into))]
    #[schemars(schema_with = "crate::schemars::scheme")]
    scheme: Option<Scheme>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(setter(into))]
    #[schemars(schema_with = "crate::schemars::dns_name")]
    host: Option<Name>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(setter(into))]
    port: Option<Port>,

    /// Redirect path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(setter(into))]
    path: Option<HttpRedirectResponsePathRewrite>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HttpRedirectResponseKind {
    Permanent,
    #[default]
    Temporary,
}

impl From<HttpRedirectResponseKind> for StatusCode {
    fn from(value: HttpRedirectResponseKind) -> Self {
        match value {
            HttpRedirectResponseKind::Permanent => StatusCode::MOVED_PERMANENTLY,
            HttpRedirectResponseKind::Temporary => StatusCode::FOUND,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum HttpRedirectResponsePathRewrite {
    Full(String),
    PrefixMatch(String),
}
