use std::cell::{RefCell};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;
use kube::Resource;
use multi_map::MultiMap;
use crate::resources::{ClusterScopedRef, ClusterScopedResource, NamespaceScopedRef, NamespaceScopedResource};

pub type ClusterScopedResourceCollection<R> = ResourceCollection<R, ClusterScopedRef<R>>;

pub type NamespaceScopedResourceCollection<R> = ResourceCollection<R, NamespaceScopedRef<R>>;

#[derive(Debug)]
pub(crate) enum AnyResourceRef<R: Resource>
where R::DynamicType: 'static + Default
{
    ClusterScoped(ClusterScopedRef<R>),
    NamespaceScoped(NamespaceScopedRef<R>),
}

impl <R: Resource> Clone for AnyResourceRef<R>
where R::DynamicType: 'static + Default
{
    fn clone(&self) -> Self {
        match self {
            AnyResourceRef::ClusterScoped(ref_) => AnyResourceRef::ClusterScoped(ref_.clone()),
            AnyResourceRef::NamespaceScoped(ref_) => AnyResourceRef::NamespaceScoped(ref_.clone()),
        }
    }
}

impl <R: Resource> PartialEq for AnyResourceRef<R>
where R::DynamicType: 'static + Default
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnyResourceRef::ClusterScoped(ref1), AnyResourceRef::ClusterScoped(ref2)) => ref1 == ref2,
            (AnyResourceRef::NamespaceScoped(ref1), AnyResourceRef::NamespaceScoped(ref2)) => ref1 == ref2,
            _ => false,
        }
    }
}

impl <R: Resource> Eq for AnyResourceRef<R>
where R::DynamicType: 'static + Default
{}

impl <R: Resource> Hash for AnyResourceRef<R>
where R::DynamicType: 'static + Default
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            AnyResourceRef::ClusterScoped(ref_) => ref_.hash(state),
            AnyResourceRef::NamespaceScoped(ref_) => ref_.hash(state),
        }
    }
}


pub(crate) trait AnyResourceRefResolver<R: Resource>: Hash + Eq
    where R::DynamicType: 'static + Default
{
    fn resource_ref(resource: &R) -> AnyResourceRef<R>;
}

#[allow(private_bounds)]
#[derive(Debug)]
pub struct ResourceCollection<R: Resource, K: AnyResourceRefResolver<R>>
where R::DynamicType: 'static + Default
{
    map: RefCell<MultiMap<AnyResourceRef<R>, String, Arc<R>>>,
    ref_resolver: PhantomData<K>,
}

#[allow(private_bounds)]
impl<R: Resource, K: AnyResourceRefResolver<R>> ResourceCollection<R, K>
where R::DynamicType: 'static + Default {

    pub fn insert(&self, resource: Arc<R>) {
        let ref_ = K::resource_ref(&resource);
        let uid = resource.meta().uid.clone().expect("Resource must have a UID");
        let mut map = self.map.borrow_mut();
        map.insert(ref_, uid, resource);
    }

    pub fn remove_by_uid(&self, uid: &str) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove_alt(uid)
    }
}

impl<R: ClusterScopedResource> Default for ResourceCollection<R, ClusterScopedRef<R>>
where R::DynamicType: 'static + Default
 {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: ClusterScopedResource> ResourceCollection<R, ClusterScopedRef<R>>
where R::DynamicType: 'static + Default
{
    pub fn new() -> Self {
        Self {
            map: RefCell::default(),
            ref_resolver: PhantomData,
        }
    }

    pub fn remove_by_ref(&self, reference: &ClusterScopedRef<R>) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove(&AnyResourceRef::ClusterScoped(reference.clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ClusterScopedRef<R>, String, Arc<R>)> {
        let map = self.map.borrow();
        let items: Vec<_> = map.iter()
            .map(|(ref_, (uid, resource))| {
                if let AnyResourceRef::ClusterScoped(ref_)= ref_ {
                    (ref_.clone(), uid.clone(), resource.clone())
                } else {
                    panic!("Expected ClusterScopedRef");
                }
            })
            .collect();
        items.into_iter()
    }
}

impl<R: NamespaceScopedResource> Default for ResourceCollection<R, NamespaceScopedRef<R>>
where R::DynamicType: 'static + Default
 {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: NamespaceScopedResource> ResourceCollection<R, NamespaceScopedRef<R>>
where R::DynamicType: 'static + Default
{
    pub fn new() -> Self {
        Self {
            map: RefCell::default(),
            ref_resolver: PhantomData,
        }
    }

    pub fn remove_by_ref(&self, reference: &NamespaceScopedRef<R>) -> Option<Arc<R>> {
        let mut map = self.map.borrow_mut();
        map.remove(&AnyResourceRef::NamespaceScoped(reference.clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item = (NamespaceScopedRef<R>, String, Arc<R>)> {
        let map = self.map.borrow();
        let items: Vec<_> = map.iter()
            .map(|(ref_, (uid, resource))| {
                if let AnyResourceRef::NamespaceScoped(ref_)= ref_ {
                    (ref_.clone(), uid.clone(), resource.clone())
                } else {
                    panic!("Expected NamespaceScopedRef");
                }
            })
            .collect();
        items.into_iter()
    }
}
