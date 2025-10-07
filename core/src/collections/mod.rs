use dashmap::DashMap;
use std::hash::Hash;
use tokio::sync::broadcast::{Sender, channel};

#[derive(PartialEq, Debug, Clone)]
pub enum CollectionEvent<K, V>
where
    K: PartialEq,
    V: PartialEq,
{
    Inserted { key: K, value: V },
    Updated { key: K, old_value: V, new_value: V },
    Removed { key: K, value: V },
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RecvError(tokio::sync::broadcast::error::RecvError);

#[derive(Debug)]
pub struct Receiver<K, V, E>
where
    K: Hash + Eq + Clone,
    V: PartialEq + Clone,
    E: From<CollectionEvent<K, V>> + Clone,
{
    inner: tokio::sync::broadcast::Receiver<E>,
    k_data: std::marker::PhantomData<K>,
    v_data: std::marker::PhantomData<V>,
}

impl<K, V, E> From<tokio::sync::broadcast::Receiver<E>> for Receiver<K, V, E>
where
    K: Hash + Eq + Clone,
    V: PartialEq + Clone,
    E: From<CollectionEvent<K, V>> + Clone,
{
    fn from(inner: tokio::sync::broadcast::Receiver<E>) -> Self {
        Self {
            inner,
            k_data: std::marker::PhantomData,
            v_data: std::marker::PhantomData,
        }
    }
}

impl<K, V, E> Receiver<K, V, E>
where
    K: Hash + Eq + Clone,
    V: PartialEq + Clone,
    E: From<CollectionEvent<K, V>> + Clone,
{
    pub async fn recv(&mut self) -> Result<E, RecvError> {
        match self.inner.recv().await {
            Ok(event) => Ok(event),
            Err(e) => Err(RecvError(e)),
        }
    }
}

#[derive(Debug)]
pub struct NotifyingCollection<K, V, E>
where
    K: Hash + Eq + Clone,
    V: PartialEq + Clone,
    E: From<CollectionEvent<K, V>> + Clone,
{
    map: DashMap<K, V>,
    tx: Sender<E>,
    #[allow(dead_code)]
    rx: Receiver<K, V, E>,
}

impl<K, V, E> NotifyingCollection<K, V, E>
where
    K: Hash + Eq + Clone,
    V: PartialEq + Clone,
    E: From<CollectionEvent<K, V>> + Clone,
{
    pub fn new(channel_capacity: usize) -> Self {
        let (tx, rx) = channel(channel_capacity);
        Self {
            map: DashMap::new(),
            tx,
            rx: rx.into(),
        }
    }

    pub fn insert(&self, key: K, value: V) {
        match self.map.insert(key.clone(), value.clone()) {
            Some(old_value) if old_value != value => {
                let _ = self.tx.send(
                    CollectionEvent::Updated {
                        key,
                        old_value,
                        new_value: value,
                    }
                    .into(),
                );
            }
            None => {
                let _ = self
                    .tx
                    .send(CollectionEvent::Inserted { key, value }.into());
            }
            _ => {}
        }
    }

    pub fn remove(&self, key: &K) {
        if let Some((key, value)) = self.map.remove(key) {
            let _ = self.tx.send(CollectionEvent::Removed { key, value }.into());
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.map.get(key).map(|v| v.value().clone())
    }

    pub fn subscribe(&self) -> Receiver<K, V, E> {
        self.tx.subscribe().into()
    }
}
