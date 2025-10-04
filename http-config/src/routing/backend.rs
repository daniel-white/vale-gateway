use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct BackendRef(String);
