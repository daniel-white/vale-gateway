use crate::http::filter::access_control::AccessControlFilterRef;
use crate::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use crate::http::filter::header_modifier::{
    HeaderModifierFilter, RequestHeaderModifierFilter, ResponseHeaderModifierFilter,
};
use crate::http::filter::redirect_response::RedirectResponseFilter;
use crate::http::filter::static_response::StaticResponseFilterRef;
use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleFilter {
    AccessControl(AccessControlRuleFilter),
    RequestHeaderModifier(RequestHeaderModifierRuleFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRuleFilter),
    RedirectResponse(RedirectResponseRuleFilter),
    StaticResponse(StaticResponseRuleFilter),
    BackendUriRewriter(BackendUriRewriterRuleFilter),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleBackendFilter {
    RequestHeaderModifier(RequestHeaderModifierRuleFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRuleFilter),
    BackendUriRewriter(BackendUriRewriterRuleFilter),
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlRuleFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierRuleFilter(RequestHeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierRuleFilter(ResponseHeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RedirectResponseRuleFilter(RedirectResponseFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseRuleFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct BackendUriRewriterRuleFilter(BackendUriRewriterFilter);
