use crate::http::rewriting::uri::UriRewriter;
use getset::{CopyGetters, Getters};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Getters, CopyGetters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RedirectResponseFilter {
    #[getset(get_copy = "pub")]
    #[serde(with = "http_serde_ext::status_code")]
    status_code: StatusCode,

    #[getset(get = "pub")]
    #[serde(flatten)]
    uri: UriRewriter,
}
