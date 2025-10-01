use derive_more::{Deref, DerefMut, From};
use gateway_api::common::HTTPRouteUrlRewrite as HTTPRouteUrlRewriteInner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "config")]
pub mod config;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HTTPRouteUrlRewrite(HTTPRouteUrlRewriteInner);
