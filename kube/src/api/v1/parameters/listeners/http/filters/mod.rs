use crate::api::v1::common::Ref;
use derive_more::{Deref, DerefMut, From};
use gateway_api::common::{
    HTTPRouteUrlRewrite as HTTPRouteUrlRewriteInner, HeaderModifier as HeaderModifierInner,
    RequestRedirect as RequestRedirectInner,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod access_control;
pub mod client_addr;
pub mod error_response;
pub mod static_response;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HeaderModifier(HeaderModifierInner);

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct RequestRedirect(RequestRedirectInner);

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HTTPRouteUrlRewrite(HTTPRouteUrlRewriteInner);

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHttpListenerFilters {
    #[serde(rename = "type")]
    pub r#type: GatewayHttpListenerFilterType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_header_modifier: Option<HeaderModifier>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_header_modifier: Option<HeaderModifier>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_redirect: Option<RequestRedirect>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_control: Option<Ref>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_address: Option<Ref>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_response: Option<Ref>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GatewayHttpListenerFilterType {
    RequestHeaderModifier,
    ResponseHeaderModifier,
    RequestRedirect,
    ClientAddress,
    AccessControl,
    ErrorResponse,
}
