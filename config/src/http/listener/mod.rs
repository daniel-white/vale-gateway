pub mod policy;

use crate::http::backend::BackendRef;
use crate::http::listener::policy::ListenerPolicies;
use crate::http::route::RouteRef;
use derive_more::{Deref, From};
use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::collections::{CollectionEvent, NotifyingCollection};
use vg_core::net::Port;
use crate::http::filter::access_control::AccessControlFilterRef;
use crate::http::filter::{GatewayFilter, SharedFilterRef};
use crate::http::filter::static_response::StaticResponseFilterRef;

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct ListenerRef(String);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, CopyGetters)]
#[serde(rename_all = "camelCase")]
pub struct ListenerProtocols {
    #[getset(get_copy = "pub")]
    http: Option<Port>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, Getters)]
#[serde(rename_all = "camelCase")]
pub struct ListenerTransport {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    ref_: ListenerRef,
    #[getset(get = "pub")]
    protocols: ListenerProtocols,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, Getters)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    ref_: ListenerRef,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "ListenerPolicies::is_default")]
    policies: ListenerPolicies,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<GatewayFilter>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    shared_filter_refs: Vec<SharedFilterRef>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_refs: Vec<RouteRef>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    backend_refs: Vec<BackendRef>,
}



#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListenerCollectionEvent {
    Added(ListenerRef),
    Modified(ListenerRef),
    Removed(ListenerRef),
}

impl From<CollectionEvent<ListenerRef, Listener>> for ListenerCollectionEvent {
    fn from(event: CollectionEvent<ListenerRef, Listener>) -> Self {
        match event {
            CollectionEvent::Inserted { key, .. } => Self::Added(key),
            CollectionEvent::Updated { key, .. } => Self::Modified(key),
            CollectionEvent::Removed { key, .. } => Self::Removed(key),
        }
    }
}

#[derive(Debug, Deref, From)]
pub struct ListenerCollection(NotifyingCollection<ListenerRef, Listener, ListenerCollectionEvent>);

impl ListenerCollection {
    pub fn new(channel_capacity: usize) -> Self {
        NotifyingCollection::new(channel_capacity).into()
    }
}
