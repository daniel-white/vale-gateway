use kube_core::Resource;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

pub trait ResourceRef<R>: Clone + Hash + PartialEq + Eq + Debug
where
    R: Resource,
    R::DynamicType: 'static + Default,
{
}

pub trait ResourceCollection<K, R>: Default
where
    K: ResourceRef<R>,
    R: Resource,
    R::DynamicType: 'static + Default,
{
    fn insert(&self, resource: R);

    fn remove_by_uid(&self, uid: &str) -> Option<Arc<R>>;

    fn remove_by_ref(&self, ref_: &K) -> Option<Arc<R>>;

    fn iter(&self) -> impl Iterator<Item = (K, String, Arc<R>)>;
}
