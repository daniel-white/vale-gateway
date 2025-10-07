use derive_more::From;
use getset::{CloneGetters, Getters};
use http::Uri;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailFormat {
    #[getset(get_clone = "pub")]
    #[serde(with = "http_serde_ext::uri::option")]
    authority: Option<Uri>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "generator", rename_all = "camelCase")]
pub enum Format {
    Empty,
    #[default]
    Html,
    ProblemDetail(ProblemDetailFormat),
}

#[derive(
    Default,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
    Getters,
    CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponsePolicy {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    #[serde(flatten)]
    format: Format,
}

impl ErrorResponsePolicy {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
