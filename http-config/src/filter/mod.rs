use crate::filter::access_control::AccessControlFilterRef;
use crate::filter::error_response::ErrorResponseFilterRef;
use crate::filter::header_modifier::HeaderModifierFilter;
use crate::filter::redirect_response::RedirectResponseFilter;
use crate::filter::static_response::StaticResponseFilterRef;
use crate::filter::upstream_uri_rewrite::UpstreamUriRewriteFilter;
use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

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
pub struct AccessControlRouteFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlGatewayFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseRouteFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: ErrorResponseFilterRef,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseGatewayFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: ErrorResponseFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierRouteFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierGatewayFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierRouteFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierGatewayFilter(HeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RedirectResponseRouteFilter(RedirectResponseFilter);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseRouteFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct UpstreamUriRewriteRouteFilter(UpstreamUriRewriteFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RouteFilter {
    AccessControl(AccessControlRouteFilter),
    ErrorResponse(ErrorResponseRouteFilter),
    RequestHeaderModifier(RequestHeaderModifierRouteFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRouteFilter),
    RedirectResponse(RedirectResponseRouteFilter),
    StaticResponse(StaticResponseRouteFilter),
    UpstreamUriRewrite(UpstreamUriRewriteRouteFilter),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GatewayFilter {
    AccessControl(AccessControlGatewayFilter),
    ErrorResponse(ErrorResponseGatewayFilter),
    RequestHeaderModifier(RequestHeaderModifierGatewayFilter),
    ResponseHeaderModifier(ResponseHeaderModifierGatewayFilter),
    StaticResponse(StaticResponseGatewayFilter),
}
