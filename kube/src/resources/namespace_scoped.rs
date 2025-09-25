use crate::resources::collection::{AnyResourceRef, AnyResourceRefResolver};
use crate::resources::kinds::ResourceKind;
use getset::Getters;
use kube::Resource;
use std::fmt::{Display, Formatter, Write};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub trait NamespaceScopedResource: Resource {
    fn resource_ref<R: NamespaceScopedResource>(&self) -> NamespaceScopedRef<R>
    where
        R::DynamicType: 'static + Default,
    {
        let meta = self.meta();
        let namespace = meta
            .namespace
            .clone()
            .expect("NamespaceScopedResource must have a namespace");
        let name = meta
            .name
            .clone()
            .expect("NamespaceScopedResource must have a name");

        NamespaceScopedRef::new_named(&namespace, &name)
    }
}

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
    pub fn new(resource: &R) -> Self {
        let meta = resource.meta();
        let namespace = meta
            .namespace
            .as_ref()
            .expect("NamespaceScopedResource must have a namespace");
        let name = meta
            .name
            .as_ref()
            .expect("NamespaceScopedResource must have a name");

        Self::new_named(namespace, name)
    }

    pub fn new_named(namespace: &str, name: &str) -> Self {
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

impl<R: NamespaceScopedResource> AnyResourceRefResolver<R> for NamespaceScopedRef<R>
where
    R::DynamicType: 'static + Default,
{
    fn resource_ref(resource: &R) -> AnyResourceRef<R> {
        AnyResourceRef::NamespaceScoped(resource.resource_ref::<R>())
    }
}
