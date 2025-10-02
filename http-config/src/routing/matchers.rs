use getset::{CloneGetters, Getters};
use http::{HeaderName, HeaderValue, Method};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
#[serde(rename_all = "camelCase")]
pub enum HeaderValueMatcher {
    #[serde(with = "http_serde_ext::header_value")]
    Exact(HeaderValue),
    RegularExpression(String),
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HeaderMatcher {
    #[getset(get_clone = "pub")]
    #[serde(with = "http_serde_ext::header_name")]
    name: HeaderName,

    #[serde(flatten)]
    #[getset(get = "pub")]
    value: HeaderValueMatcher,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct HeadersMatcher {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[getset(get = "pub")]
    headers: Vec<HeaderMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
#[serde(rename_all = "camelCase")]
pub enum HostHeaderValueMatcher {
    Exact(String),
    InZone(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct HostHeaderMatcher {
    #[serde(flatten)]
    #[getset(get = "pub")]
    matchers: Vec<HostHeaderValueMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, CloneGetters, TypedBuilder)]
pub struct MethodMatcher {
    #[getset(get_clone = "pub")]
    #[serde(with = "http_serde_ext::method")]
    method: Method,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
#[serde(rename_all = "camelCase")]
pub enum PathMatcher {
    Exact(String),
    Prefix(String),
    RegularExpression(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
#[serde(rename_all = "camelCase")]
pub enum QueryParamValueMatcher {
    Exact(String),
    RegularExpression(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct QueryParamMatcher {
    #[getset(get = "pub")]
    name: String,

    #[serde(flatten)]
    #[getset(get = "pub")]
    value: QueryParamValueMatcher,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct QueryParamsMatcher {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    #[getset(get = "pub")]
    query_params: Vec<QueryParamMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RequestMatcher {
    #[getset(get = "pub")]
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    method: Option<MethodMatcher>,
    #[getset(get = "pub")]
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PathMatcher>,
    #[getset(get = "pub")]
    #[serde(flatten)]
    headers: HeadersMatcher,
    #[getset(get = "pub")]
    #[serde(flatten)]
    query_params: QueryParamsMatcher,
}
