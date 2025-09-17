use std::fmt::{Display, Formatter, Write};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use getset::Getters;
use kube::Resource;
use crate::resources::collection::{AnyResourceRef, AnyResourceRefResolver};
use crate::resources::kinds::{ResourceKind};

pub trait ClusterScopedResource : Resource
{
    fn resource_ref<R: ClusterScopedResource>(&self) -> ClusterScopedRef<R>
        where R::DynamicType: 'static + Default
    {
        let meta = self.meta();
        let name = meta.name.clone().expect("ClusterScopedResource must have a name");
        
        ClusterScopedRef::new_named(&name)
    }
}

#[derive(Debug, Getters)]
pub struct ClusterScopedRef<R: Resource>
where R::DynamicType: 'static + Default
{
    kind: ResourceKind<R>,
    name: Arc<String>,
}

impl <R: Resource> Clone for ClusterScopedRef<R>
    where R::DynamicType: 'static + Default
{
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            name: self.name.clone(),
        }
    }
}

impl <R: Resource> ClusterScopedRef<R>
    where R::DynamicType: 'static + Default
{
    pub fn new(resource: &R) -> Self {
        let meta = resource.meta();
        let name = meta.name.clone().expect("ClusterScopedResource must have a name");

        Self::new_named(&name)
    }

    pub fn new_named(name: &str) -> Self {
        Self {
            kind: Default::default(),
            name: Arc::new(name.to_string()),
        }
    }

    pub fn group(&self) -> Option<&str> {
        self.kind.group()
    }

    pub fn kind(&self) -> &str {
        self.kind.kind()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl <R: Resource> PartialEq for ClusterScopedRef<R>
    where R::DynamicType: 'static + Default
{
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.name == other.name
    }
}

impl <R: Resource> Eq for ClusterScopedRef<R>
    where R::DynamicType: 'static + Default
{}

impl <R: Resource> Hash for ClusterScopedRef<R>
    where R::DynamicType: 'static + Default
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.name.hash(state);
    }
}



impl <R: Resource> Display for ClusterScopedRef<R>
    where R::DynamicType: 'static + Default
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

impl<R: ClusterScopedResource> AnyResourceRefResolver<R> for ClusterScopedRef<R>
where R::DynamicType: 'static + Default
{
    fn resource_ref(resource: &R) -> AnyResourceRef<R>
    {
        AnyResourceRef::ClusterScoped(resource.resource_ref::<R>())
    }
}

