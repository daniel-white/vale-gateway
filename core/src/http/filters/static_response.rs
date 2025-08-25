use getset::Getters;
use http::{HeaderValue, StatusCode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use typed_builder::TypedBuilder;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct HttpStaticResponseFilterKey(String);

impl<S: AsRef<str>> From<S> for HttpStaticResponseFilterKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Hash, Eq)]
#[serde(transparent)]
pub struct HttpStaticResponseBodyKey(String);

impl<S: AsRef<str>> From<S> for HttpStaticResponseBodyKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

impl Display for HttpStaticResponseFilterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for &HttpStaticResponseBodyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder, Getters,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpStaticResponseFilterRef {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpStaticResponseFilterKey,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder, Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpStaticResponseFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpStaticResponseFilterKey,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    #[serde(with = "http_serde_ext::status_code")]
    #[schemars(schema_with = "crate::schemars::status_code")]
    status_code: StatusCode,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub")]
    #[builder(default, setter(into))]
    body: Option<HttpStaticResponseBody>,
}

#[derive(
    Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Getters, TypedBuilder, Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpStaticResponseBody {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    key: HttpStaticResponseBodyKey,

    #[getset(get = "pub")]
    #[builder(setter(into))]
    #[serde(with = "http_serde_ext::header_value")]
    #[schemars(schema_with = "crate::schemars::http_header_value")]
    content_type: HeaderValue,
}
