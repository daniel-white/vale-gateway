use crate::resources::kinds::ResourceKind;
use getset::Getters;
use kube::Resource;
use multi_map::MultiMap;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter, Write};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub trait NamespaceScopedResource: Resource {}

#[derive(Debug, Getters)]
pub struct NamespaceScopedRef<R: Resource>
where
    R::DynamicType: 'static + Default,
{
    kind: ResourceKind<R>,
    namespace: Arc<String>,
    name: Arc<String>,
}

impl<R: Resource> NamespaceScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    pub fn new(namespace: &str, name: &str) -> Self {
        Self {
            kind: Default::default(),
            namespace: Arc::new(namespace.to_string()),
            name: Arc::new(name.to_string()),
        }
    }

    pub fn group(&self) -> Option<&str> {
        self.kind.group()
    }

    pub fn kind(&self) -> &str {
        self.kind.kind()
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<R: Resource> From<&R> for NamespaceScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn from(value: &R) -> Self {
        let meta = value.meta();
        let namespace = meta
            .namespace
            .as_ref()
            .expect("NamespaceScopedResource must have a namespace");
        let name = meta
            .name
            .as_ref()
            .expect("NamespaceScopedResource must have a name");

        Self::new(namespace, name)
    }
}

impl<R: Resource> Clone for NamespaceScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            namespace: self.namespace.clone(),
            name: self.name.clone(),
        }
    }
}

impl<R: Resource> PartialEq for NamespaceScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.namespace == other.namespace && self.name == other.name
    }
}

impl<R: Resource> Eq for NamespaceScopedRef<R> where R::DynamicType: 'static + Default {}

impl<R: Resource> Hash for NamespaceScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.namespace.hash(state);
        self.name.hash(state);
    }
}

impl<R: Resource> Display for NamespaceScopedRef<R>
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
        f.write_char('.')?;
        f.write_str(self.namespace())?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct NamespaceScopedResourceCollection<K, R>
where
    K: Clone + Hash + PartialEq + Eq + Debug,
    R: NamespaceScopedResource,
    R::DynamicType: 'static + Default,
{
    map: RefCell<MultiMap<K, String, Arc<R>>>,
}

impl<K, R> Default for NamespaceScopedResourceCollection<K, R>
where
    K: Clone + Hash + PartialEq + Eq + Debug + From<NamespaceScopedRef<R>>,
    R: NamespaceScopedResource,
    R::DynamicType: 'static + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, R> NamespaceScopedResourceCollection<K, R>
where
    K: Clone + Hash + PartialEq + Eq + Debug + From<NamespaceScopedRef<R>>,
    R: NamespaceScopedResource,
    R::DynamicType: 'static + Default,
{
    pub fn new() -> Self {
        Self {
            map: RefCell::default(),
        }
    }

    pub fn insert(&self, resource: R) {
        let ref_: NamespaceScopedRef<R> = (&resource).into();
        let ref_: K = ref_.into();
        let uid = resource
            .meta()
            .uid
            .as_ref()
            .expect("Resource must have a UID")
            .clone();
        let arc = Arc::new(resource);
        let mut map = self.map.borrow_mut();
        map.insert(ref_, uid, arc);
    }

    pub fn remove_by_uid(&self, uid: &str) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove_alt(uid)
    }

    pub fn remove_by_ref(&self, ref_: &K) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove(ref_)
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, String, Arc<R>)> {
        let map = self.map.borrow();
        let items: Vec<_> = map
            .iter()
            .map(|(ref_, (uid, resource))| (ref_.clone(), uid.clone(), resource.clone()))
            .collect();
        items.into_iter()
    }
}
