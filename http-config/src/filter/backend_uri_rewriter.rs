use crate::rewriting::uri::UriRewriter;
use getset::Getters;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Getters)]
#[serde(rename_all = "camelCase")]
pub struct BackendUriRewriterFilter {
    #[getset(get = "pub")]
    #[serde(flatten)]
    uri: UriRewriter,
}
