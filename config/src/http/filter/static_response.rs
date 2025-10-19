use std::ops::Deref;
use derive_more::{From, FromStr};
use getset::{CloneGetters, CopyGetters, Getters};
use http::{StatusCode, Uri};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_core::http::content_type::ContentTypeBuf;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash, From, FromStr)]
#[serde(transparent)]
pub struct StaticResponseFilterRef(String);

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
    Getters,
    CopyGetters,
    CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseFilter {
    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    #[serde(with = "http_serde_ext::status_code")]
    status_code: StatusCode,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    body: Option<Body>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
    Getters,
    CloneGetters,
    CopyGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    content_type: ContentTypeBuf,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    #[serde(flatten)]
    content: BodyContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum BodyContent {
    Text(String),
    Binary(Arc<[u8]>),
    #[serde(with = "http_serde_ext::uri")]
    Remote(Uri),
}

impl From<String> for BodyContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<Arc<[u8]>> for BodyContent {
    fn from(value: Arc<[u8]>) -> Self {
        Self::Binary(value)
    }
}

impl From<&[u8]> for BodyContent {
    fn from(value: &[u8]) -> Self {
        Self::Binary(Arc::from(value))
    }
}

impl From<Uri> for BodyContent {
    fn from(value: Uri) -> Self {
        Self::Remote(value)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseGatewayFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseSharedFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,

    #[getset(get = "pub")]
    #[serde(flatten)]
    filter: StaticResponseFilter
}

impl Deref for StaticResponseSharedFilter {
    type Target = StaticResponseFilter;

    fn deref(&self) -> &Self::Target {
        self.filter()
    }
}
