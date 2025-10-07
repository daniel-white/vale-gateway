use derive_more::{Deref, From};
use getset::Getters;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::collections::{CollectionEvent, NotifyingCollection};

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, From)]
#[serde(transparent)]
pub struct BackendRef(String);

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, TypedBuilder, Getters)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    #[getset(get = "pub")]
    #[serde(rename = "ref")]
    ref_: BackendRef,
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
