use crate::filter::access_control::AccessControlFilterRef;
use crate::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use crate::filter::error_response::ErrorResponseFilterRef;
use crate::filter::header_modifier::HeaderModifierFilter;
use crate::filter::redirect_response::RedirectResponseFilter;
use crate::filter::static_response::StaticResponseFilterRef;
use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleFilter {
    AccessControl(AccessControlRuleFilter),
    ErrorResponse(ErrorResponseRuleFilter),
    RequestHeaderModifier(RequestHeaderModifierRuleFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRuleFilter),
    RedirectResponse(RedirectResponseRuleFilter),
    StaticResponse(StaticResponseRuleFilter),
    BackendUriRewriter(BackendUriRewriterRuleFilter),
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlRuleFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseRuleFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: ErrorResponseFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierRuleFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierRuleFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RedirectResponseRuleFilter(RedirectResponseFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseRuleFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct BackendUriRewriterRuleFilter(BackendUriRewriterFilter);
