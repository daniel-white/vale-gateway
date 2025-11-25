use crate::resources::ResourceRef;
use crate::resources::common::ResourceCollection;
use crate::resources::kind::ResourceKind;
use getset::Getters;
use kube::Resource;
use multi_map::MultiMap;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter, Write};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub trait ClusterScopedResource: Resource {}

#[derive(Getters)]
pub struct ClusterScopedRef<R: ClusterScopedResource>
where
    R::DynamicType: 'static + Default,
{
    kind: ResourceKind<R>,
    name: Arc<String>,
}

impl<R: ClusterScopedResource> ResourceRef<R> for ClusterScopedRef<R>
where
    R: Resource,
    R::DynamicType: 'static + Default,
{
}

impl<R: ClusterScopedResource> Debug for ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusterScopedRef")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .finish()
    }
}

impl<R: ClusterScopedResource> Clone for ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            name: self.name.clone(),
        }
    }
}

impl<R: ClusterScopedResource> ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    #[must_use] 
    pub fn new(name: &str) -> Self {
        Self {
            kind: Default::default(),
            name: Arc::new(name.to_string()),
        }
    }

    #[must_use] 
    pub fn group(&self) -> Option<&str> {
        self.kind.group()
    }

    #[must_use] 
    pub fn kind(&self) -> &str {
        self.kind.kind()
    }

    #[must_use] 
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<R: ClusterScopedResource> From<&R> for ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn from(value: &R) -> Self {
        let meta = value.meta();
        let name = meta.name.clone().expect("ClusterScopedResource must have a name");

        Self::new(&name)
    }
}

impl<R: ClusterScopedResource> PartialEq for ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.name == other.name
    }
}

impl<R: ClusterScopedResource> Eq for ClusterScopedRef<R> where R::DynamicType: 'static + Default {}

impl<R: ClusterScopedResource> Hash for ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.name.hash(state);
    }
}

impl<R: ClusterScopedResource> Display for ClusterScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kind())?;
        f.write_char('.')?;
        if let Some(group) = self.group() {
            f.write_str(group)?;
        }
        f.write_char('/')?;
        f.write_str(self.name())?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ClusterScopedResourceCollection<K, R>
where
    K: ResourceRef<R>,
    R: ClusterScopedResource,
    R::DynamicType: 'static + Default,
{
    map: RefCell<MultiMap<K, String, Arc<R>>>,
}

impl<K, R> Default for ClusterScopedResourceCollection<K, R>
where
    K: ResourceRef<R>,
    R: ClusterScopedResource,
    R::DynamicType: 'static + Default,
{
    fn default() -> Self {
        Self {
            map: RefCell::default(),
        }
    }
}

impl<K, R> ResourceCollection<K, R> for ClusterScopedResourceCollection<K, R>
where
    K: ResourceRef<R> + From<ClusterScopedRef<R>>,
    R: ClusterScopedResource,
    R::DynamicType: 'static + Default,
{
    fn insert(&self, resource: R) {
        let ref_: ClusterScopedRef<R> = (&resource).into();
        let ref_: K = ref_.into();
        let uid = resource.meta().uid.as_ref().expect("Resource must have a UID").clone();
        let arc = Arc::new(resource);
        let mut map = self.map.borrow_mut();
        map.insert(ref_, uid, arc);
    }

    fn remove_by_uid(&self, uid: &str) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove_alt(uid)
    }

    fn remove_by_ref(&self, ref_: &K) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove(ref_)
    }

    fn iter(&self) -> impl Iterator<Item = (K, String, Arc<R>)> {
        let map = self.map.borrow();
        let items: Vec<_> = map
            .iter()
            .map(|(ref_, (uid, resource))| (ref_.clone(), uid.clone(), resource.clone()))
            .collect();
        items.into_iter()
    }
}
