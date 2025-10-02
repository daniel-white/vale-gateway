use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use crate::filter::access_control::AccessControlFilterRef;
use crate::filter::error_response::ErrorResponseFilterRef;
use crate::filter::header_modifier::HeaderModifierFilter;
use crate::filter::redirect_response::RedirectResponseFilter;
use crate::filter::static_response::StaticResponseFilterRef;
use crate::filter::upstream_uri_rewrite::UpstreamUriRewriteFilter;

pub mod access_control;
pub mod client_addr;
pub mod error_response;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;
pub mod upstream_uri_rewrite;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlRequestFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseRequestFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: ErrorResponseFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From,
)]
#[serde(transparent)]
pub struct RequestHeaderModifierRequestFilter(HeaderModifierFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From,
)]
#[serde(transparent)]
pub struct ResponseHeaderModifierRequestFilter(HeaderModifierFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From,
)]
#[serde(transparent)]
pub struct RedirectResponseRequestFilter(RedirectResponseFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseRequestFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From,
)]
#[serde(transparent)]
pub struct UpstreamUriRewriteRequestFilter(UpstreamUriRewriteFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From
)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RequestFilter {
    AccessControl(AccessControlRequestFilter),
    ErrorResponse(ErrorResponseRequestFilter),
    RequestHeaderModifier(RequestHeaderModifierRequestFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRequestFilter),
    RedirectResponse(RedirectResponseRequestFilter),
    StaticResponse(StaticResponseRequestFilter),
    UpstreamUriRewrite(UpstreamUriRewriteRequestFilter),
}
