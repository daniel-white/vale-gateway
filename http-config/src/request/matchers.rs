use getset::{CloneGetters, Getters};
use http::{HeaderName, HeaderValue, Method};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum HeaderValueMatcher {
    #[serde(with = "http_serde_ext::header_value")]
    Exact(HeaderValue),
    RegularExpression(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
pub struct HeaderMatcher {
    #[getset(get = "pub")]
    #[serde(with = "http_serde_ext::header_name")]
    name: HeaderName,

    #[getset(get = "pub")]
    value: HeaderValueMatcher,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
pub struct HeadersMatcher {
    #[serde(flatten)]
    #[getset(get = "pub")]
    matchers: Vec<HeaderMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum HostHeaderValueMatcher {
    Exact(String),
    InZone(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
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
#[serde(tag = "type", content = "value")]
pub enum PathMatcher {
    Exact(String),
    Prefix(String),
    RegularExpression(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum QueryParamValueMatcher {
    Exact(String),
    RegularExpression(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
pub struct QueryParamMatcher {
    #[getset(get = "pub")]
    name: String,

    #[getset(get = "pub")]
    value: QueryParamValueMatcher,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
pub struct QueryParamsMatcher {
    #[serde(flatten)]
    #[getset(get = "pub")]
    matchers: Vec<QueryParamMatcher>,
}
