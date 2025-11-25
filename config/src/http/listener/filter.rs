use crate::http::filter::access_control::AccessControlFilterRef;
use crate::http::filter::header_modifier::{RequestHeaderModifierFilter, ResponseHeaderModifierFilter};
use crate::http::filter::static_response::StaticResponseFilterRef;
use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ListenerFilter {
    AccessControl(AccessControlListenerFilter),
    RequestHeaderModifier(RequestHeaderModifierListenerFilter),
    ResponseHeaderModifier(ResponseHeaderModifierListenerFilter),
    StaticResponse(StaticResponseListenerFilter),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlListenerFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierListenerFilter(RequestHeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierListenerFilter(ResponseHeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseListenerFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
}
