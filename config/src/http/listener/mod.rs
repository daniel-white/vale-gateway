pub mod filter;
pub mod policy;

use crate::http::filter::SharedFilterRef;
use crate::http::gateway::GatewayRef;
use crate::http::listener::filter::ListenerFilter;
use crate::http::listener::policy::ListenerPolicies;
use crate::http::route::RouteRef;
use derive_more::{Deref, From, TryUnwrap};
use getset::{CloneGetters, CopyGetters, Getters};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use typed_builder::TypedBuilder;
use vg_core::collections::{CollectionEvent, NotifyingCollection};
use vg_core::net::Port;

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct ListenerRef(String);

impl Display for ListenerRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
    Getters,
    CloneGetters,
    CopyGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    ref_: ListenerRef,

    #[getset(get = "pub")]
    protocol: ListenerProtocol,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "ListenerPolicies::is_default")]
    policies: ListenerPolicies,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<ListenerFilter>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_refs: Vec<RouteRef>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TryUnwrap)]
#[serde(rename_all = "lowercase")]
pub enum ListenerProtocol {
    HTTP(Port),
}
