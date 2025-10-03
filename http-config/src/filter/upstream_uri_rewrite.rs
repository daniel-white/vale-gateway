use crate::rewriting::uri::UriRewriter;
use derive_more::{Deref, From};
use getset::Getters;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Getters)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamUriRewriteFilter {
    #[getset(get = "pub")]
    #[serde(flatten)]
    uri: UriRewriter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
#[serde(transparent)]
pub struct UpstreamUriRewriteRouteRuleFilter(UpstreamUriRewriteFilter);
