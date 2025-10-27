use crate::http::backend::BackendRef;
use crate::http::filter::SharedFilterRef;
use crate::http::listener::Listener;
use derive_more::From;
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::sync::Arc;
use typed_builder::TypedBuilder;

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct GatewayRef(String);

impl Display for GatewayRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListenerProtocol {
    HTTP,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TypedBuilder, Getters, CloneGetters)]
#[serde(rename_all = "camelCase")]
pub struct Gateway {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    ref_: GatewayRef,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    listeners: Vec<Arc<Listener>>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<GatewayFilter>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    shared_filter_refs: Vec<SharedFilterRef>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    backend_refs: Vec<BackendRef>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_refs: Vec<crate::http::route::RouteRef>,
}

// Placeholder for GatewayFilter - will be defined in future tasks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayFilter {
    // Placeholder structure - will be expanded in future tasks
    pub name: String,
}
