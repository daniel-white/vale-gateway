use crate::filter::access_control::{
    AccessControlGatewayFilter, AccessControlRouteRuleFilter,
};
use crate::filter::error_response::{
    ErrorResponseGatewayFilter, ErrorResponseRouteRuleFilter,
};
use crate::filter::header_modifier::{
    RequestHeaderModifierGatewayFilter, RequestHeaderModifierRouteRuleFilter,
    ResponseHeaderModifierGatewayFilter, ResponseHeaderModifierRouteRuleFilter,
};
use crate::filter::redirect_response::RedirectResponseRouteRuleFilter;
use crate::filter::static_response::{
    StaticResponseGatewayFilter, StaticResponseRouteRuleFilter,
};
use crate::filter::upstream_uri_rewrite::UpstreamUriRewriteRouteRuleFilter;
use derive_more::From;
use serde::{Deserialize, Serialize};

pub mod access_control;
pub mod client_addr;
pub mod error_response;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;
pub mod upstream_uri_rewrite;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RouteRuleFilter {
    AccessControl(AccessControlRouteRuleFilter),
    ErrorResponse(ErrorResponseRouteRuleFilter),
    RequestHeaderModifier(RequestHeaderModifierRouteRuleFilter),
    ResponseHeaderModifier(ResponseHeaderModifierRouteRuleFilter),
    RedirectResponse(RedirectResponseRouteRuleFilter),
    StaticResponse(StaticResponseRouteRuleFilter),
    UpstreamUriRewrite(UpstreamUriRewriteRouteRuleFilter),
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
