use crate::http::filter::access_control::AccessControlFilterRef;
use crate::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use crate::http::filter::header_modifier::{RequestHeaderModifierFilter, ResponseHeaderModifierFilter};
use crate::http::filter::redirect_response::RedirectResponseFilter;
use crate::http::filter::static_response::StaticResponseFilterRef;
use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

/// `RouteFilter` represents filters that can be applied at the Route level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RouteFilter {
    AccessControl(AccessControlRouteFilter),
    RequestHeaderModifier(RequestHeaderModifierRouteFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRouteFilter),
    RedirectResponse(RedirectResponseRouteFilter),
    StaticResponse(StaticResponseRouteFilter),
    BackendUriRewriter(BackendUriRewriterRouteFilter),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlRouteFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: AccessControlFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RequestHeaderModifierRouteFilter(RequestHeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct ResponseHeaderModifierRouteFilter(ResponseHeaderModifierFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct RedirectResponseRouteFilter(RedirectResponseFilter);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct StaticResponseRouteFilter {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: StaticResponseFilterRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct BackendUriRewriterRouteFilter(BackendUriRewriterFilter);
