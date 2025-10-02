use derive_more::{Deref, DerefMut, From};
use gateway_api::httproutes::HTTPRouteRule as HTTPRouteRuleInner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod filter;
pub mod r#match;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HTTPRouteRule(HTTPRouteRuleInner);
