use crate::http::filter::access_control::AccessControlGatewayFilter;
use crate::http::filter::header_modifier::{
    RequestHeaderModifierGatewayFilter, ResponseHeaderModifierGatewayFilter,
};
use crate::http::filter::static_response::StaticResponseGatewayFilter;
use derive_more::From;
use serde::{Deserialize, Serialize};

pub mod access_control;
pub mod backend_uri_rewriter;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GatewayFilter {
    AccessControl(AccessControlGatewayFilter),
    RequestHeaderModifier(RequestHeaderModifierGatewayFilter),
    ResponseHeaderModifier(ResponseHeaderModifierGatewayFilter),
    StaticResponse(StaticResponseGatewayFilter),
}
