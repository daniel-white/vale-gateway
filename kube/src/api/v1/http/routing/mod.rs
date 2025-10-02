use derive_more::{Deref, DerefMut, From};
use gateway_api::httproutes::{
    HTTPRouteFilter as HTTPRouteFilterInner, HTTPRouteRule as HTTPRouteRuleInner,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod r#match;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HTTPRouteRule(HTTPRouteRuleInner);

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HTTPRouteFilter(HTTPRouteFilterInner);
