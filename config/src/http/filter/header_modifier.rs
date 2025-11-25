use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use http::{HeaderMap, HeaderName};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters)]
#[serde(rename_all = "camelCase")]
pub struct HeaderModifierFilter {
    #[getset(get_clone = "pub")]
    #[serde(
        with = "http_serde_ext::header_map",
        default,
        skip_serializing_if = "HeaderMap::is_empty"
    )]
    add: HeaderMap,
    #[getset(get_clone = "pub")]
    #[serde(
        with = "http_serde_ext::header_map",
        default,
        skip_serializing_if = "HeaderMap::is_empty"
    )]
    set: HeaderMap,
    #[getset(get_clone = "pub")]
    #[serde(
        with = "http_serde_ext::header_name::vec",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    remove: Vec<HeaderName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierFilter(HeaderModifierFilter);
