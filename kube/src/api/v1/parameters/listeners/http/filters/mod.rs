use crate::api::v1::common::Ref;
use crate::api::v1::http::filter::header_modifier::HeaderModifier;
use crate::api::v1::http::filter::request_redirect::RequestRedirect;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
