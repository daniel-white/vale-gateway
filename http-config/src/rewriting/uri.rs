use getset::{CloneGetters, CopyGetters, Getters};
use http::uri::Scheme;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::net::Port;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    TypedBuilder,
    Serialize,
    Deserialize,
    Getters,
    CopyGetters,
    CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct UriRewriter {
    #[builder(setter(into))]
    #[serde(
        default,
        with = "http_serde_ext::scheme::option",
        skip_serializing_if = "Option::is_none"
    )]
    #[getset(get_clone = "pub")]
    scheme: Option<Scheme>,

    #[builder(setter(into))]
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,

    #[builder(setter(into))]
    #[getset(get_copy = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Port>,

    #[builder(setter(into))]
    #[getset(get_clone = "pub")]
    path: Option<PathRewrite>,
}

impl UriRewriter {
    pub fn is_unset(&self) -> bool {
        self.scheme.is_none() && self.host.is_none() && self.port.is_none() && self.path.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum PathRewrite {
    Full(String),
    PrefixMatch(String),
}
