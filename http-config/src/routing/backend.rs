use derive_more::From;
use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct BackendRef(String);
