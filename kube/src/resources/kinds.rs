use kube::Resource;
use ouroboros::self_referencing;
use std::borrow::Cow;
use std::fmt::Debug;
use std::hash::Hash;

pub struct ResourceKind<R: Resource>
where
    R::DynamicType: 'static + Default,
{
    inner: ResourceKindImpl<R>,
}

impl<R: Resource> Default for ResourceKind<R>
where
    R::DynamicType: 'static + Default,
{
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<R: Resource> ResourceKind<R>
where
    R::DynamicType: 'static + Default,
{
    pub fn group(&self) -> Option<&str> {
        self.inner.group()
    }

    pub fn kind(&self) -> &str {
        self.inner.kind()
    }
}

impl<R: Resource> Clone for ResourceKind<R>
where
    R::DynamicType: 'static + Default,
{
    fn clone(&self) -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<R: Resource> Debug for ResourceKind<R>
where
    R::DynamicType: 'static + Default,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceKind")
            .field("group", &self.group())
            .field("kind", &self.kind())
            .finish()
    }
}

impl<R: Resource> PartialEq for ResourceKind<R>
where
    R::DynamicType: 'static + Default,
{
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<R: Resource> Eq for ResourceKind<R> where R::DynamicType: 'static + Default {}

impl<R: Resource> Hash for ResourceKind<R>
where
    R::DynamicType: 'static + Default,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.group().hash(state);
        self.kind().hash(state);
    }
}

#[self_referencing]
struct ResourceKindImpl<R: Resource>
where
    R::DynamicType: 'static + Default,
{
    dynamic_type: R::DynamicType,
    #[borrows(dynamic_type)]
    #[covariant]
    group: Cow<'this, str>,
    #[borrows(dynamic_type)]
    #[covariant]
    kind: Cow<'this, str>,
}

impl<R: Resource> ResourceKindImpl<R>
where
    R::DynamicType: 'static + Default,
{
    pub fn group(&self) -> Option<&str> {
        let group = self.borrow_group();
        if group.is_empty() {
            None
        } else {
            Some(group.as_ref())
        }
    }

    pub fn kind(&self) -> &str {
        self.borrow_kind()
    }
}

impl<R: Resource> Default for ResourceKindImpl<R>
where
    R::DynamicType: 'static + Default,
{
    fn default() -> Self {
        ResourceKindImplBuilder {
            dynamic_type: R::DynamicType::default(),
            group_builder: |dt| R::group(dt),
            kind_builder: |dt| R::kind(dt),
        }
        .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::{assert_none, assert_some_eq_x};
    use gateway_api::gatewayclasses::GatewayClass;
    use k8s_openapi::api::core::v1::Pod;

    #[test]
    fn test_resource_kind_with_group() {
        let rk = ResourceKindImpl::<GatewayClass>::default();
        assert_some_eq_x!(rk.group(), "gateway.networking.k8s.io");
        assert_eq!(rk.kind(), "GatewayClass");
    }

    #[test]
    fn test_resource_kind_without_group() {
        let rk = ResourceKindImpl::<Pod>::default();
        assert_none!(rk.group());
        assert_eq!(rk.kind(), "Pod");
    }
}
