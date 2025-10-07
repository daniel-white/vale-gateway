use derive_more::{From, FromStr};
use getset::{CloneGetters, Getters};
use http::Uri;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash, From, FromStr)]
#[serde(transparent)]
pub struct ErrorResponseFilterRef(String);

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailErrorResponseGenerator {
    #[getset(get_clone = "pub")]
    #[serde(with = "http_serde_ext::uri::option")]
    authority: Option<Uri>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "generator", rename_all = "camelCase")]
pub enum ErrorResponseGenerator {
    Empty,
    #[default]
    Html,
    ProblemDetail(ProblemDetailErrorResponseGenerator),
}

impl From<ProblemDetailErrorResponseGenerator> for ErrorResponseGenerator {
    fn from(value: ProblemDetailErrorResponseGenerator) -> Self {
        Self::ProblemDetail(value)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, Getters, CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    #[serde(flatten)]
    generator: ErrorResponseGenerator,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseGatewayFilter {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    #[builder(setter(into))]
    ref_: ErrorResponseFilterRef,
}
