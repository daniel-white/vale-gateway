use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "config")]
pub mod config;

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum ErrorResponsePolicyFormat {
    #[default]
    Empty,
    Html,
    ProblemDetail,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponsePolicy {
    pub format: ErrorResponsePolicyFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem_detail: Option<ProblemDetailErrorResponseFormat>,
}

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailErrorResponseFormat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
}
