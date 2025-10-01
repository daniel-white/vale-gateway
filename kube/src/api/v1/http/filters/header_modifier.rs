use derive_more::{Deref, DerefMut, From};
use gateway_api::common::HeaderModifier as HeaderModifierInner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq, Deref, DerefMut, From)]
#[serde(transparent)]
pub struct HeaderModifier(HeaderModifierInner);
