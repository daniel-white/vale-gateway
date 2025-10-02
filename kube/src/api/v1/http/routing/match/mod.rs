#[cfg(feature = "config")]
pub mod config;

use derive_more::{Deref, DerefMut, From};
use gateway_api::httproutes::RouteMatch as RouteMatchInner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct RouteMatch(RouteMatchInner);
