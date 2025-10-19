use derive_more::{Deref, From};
use getset::{CloneGetters, Getters};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use typed_builder::TypedBuilder;
use vg_core::collections::{CollectionEvent, NotifyingCollection};

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct BackendRef(String);

#[derive(
    Debug, PartialEq, Eq, Clone, Serialize, Deserialize, TypedBuilder, Getters, CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    #[getset(get_clone = "pub")]
    #[serde(rename = "ref")]
    ref_: BackendRef,

    #[getset(get = "pub")]
    endpoints: Vec<BackendEndpoint>,
}

#[derive(Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct BackendEndpoint {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node: Option<String>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zone: Option<String>,

    #[getset(get = "pub")]
    addrs: Vec<IpAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendCollectionEvent {
    Added(BackendRef),
    Modified(BackendRef),
    Removed(BackendRef),
}

impl From<CollectionEvent<BackendRef, Backend>> for BackendCollectionEvent {
    fn from(event: CollectionEvent<BackendRef, Backend>) -> Self {
        match event {
            CollectionEvent::Inserted { key, .. } => Self::Added(key),
            CollectionEvent::Updated { key, .. } => Self::Modified(key),
            CollectionEvent::Removed { key, .. } => Self::Removed(key),
        }
    }
}

#[derive(Debug, Deref, From)]
pub struct BackendCollection(NotifyingCollection<BackendRef, Backend, BackendCollectionEvent>);

impl BackendCollection {
    pub fn new(channel_capacity: usize) -> Self {
        NotifyingCollection::new(channel_capacity).into()
    }
}
