use crate::http::filter::access_control::{AccessControlFilterRef, AccessControlSharedFilter};
use crate::http::filter::static_response::{StaticResponseFilterRef, StaticResponseSharedFilter};
use serde::{Deserialize, Serialize};

pub mod access_control;
pub mod backend_uri_rewriter;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SharedFilter {
    AccessControl(AccessControlSharedFilter),
    StaticResponse(StaticResponseSharedFilter),
}

impl SharedFilter {
    pub fn ref_(&self) -> SharedFilterRef {
        match self {
            SharedFilter::AccessControl(filter) => SharedFilterRef::AccessControl(filter.ref_()),
            SharedFilter::StaticResponse(filter) => SharedFilterRef::StaticResponse(filter.ref_()),
        }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind", content = "ref", rename_all = "camelCase")]
pub enum SharedFilterRef {
    AccessControl(AccessControlFilterRef),
    StaticResponse(StaticResponseFilterRef),
}
