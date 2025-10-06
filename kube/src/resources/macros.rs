#[macro_export]
macro_rules! cluster_scope {
    ($t:ty) => {
        paste::paste! {
            impl ClusterScopedResource for $t {}

            #[derive(Debug, derive_more::Deref, derive_more::From)]
            #[cfg(feature = "config")]
            pub struct [<$t Wrapper>]<'a>(&'a $t);

            #[derive(Debug, Default, derive_more::Deref, derive_more::DerefMut)]
            pub struct [<$t Collection>](ClusterScopedResourceCollection<[<$t Ref>], $t>);

            impl ResourceCollection<[<$t Ref>], $t> for [<$t Collection>] {
                fn insert(&self, resource: $t) {
                    self.0.insert(resource);
                }

                fn remove_by_uid(&self, uid: &str) -> Option<std::sync::Arc<$t>> {
                    self.0.remove_by_uid(uid)
                }

                fn remove_by_ref(&self, ref_: &[<$t Ref>]) -> Option<std::sync::Arc<$t>> {
                    self.0.remove_by_ref(ref_)
                }

                fn iter(&self) -> impl Iterator<Item = ([<$t Ref>], String, std::sync::Arc<$t>)> {
                    self.0.iter()
                }
            }

            #[derive(Clone, Debug, Hash, PartialEq, Eq, derive_more::Deref, derive_more::From)]
            pub struct [<$t Ref>](ClusterScopedRef<$t>);

            impl ResourceRef<$t> for [<$t Ref>] {}

            impl [<$t Ref>] {
                pub fn new(name: &str) -> Self {
                    Self(ClusterScopedRef::new(name))
                }
            }

            impl From<&$t> for [<$t Ref>] {
                fn from(r: &$t) -> Self {
                    Self(r.into())
                }
            }

            #[derive(Debug, Default, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
            pub struct [<$t RefCollection>](std::collections::HashSet<[<$t Ref>]>);

            impl std::iter::FromIterator<[<$t Ref>]> for [<$t RefCollection>] {
                fn from_iter<T: IntoIterator<Item = [<$t Ref>]>>(iter: T) -> Self {
                    Self(iter.into_iter().collect())
                }
            }
        }
    };
}

#[macro_export]
macro_rules! namespace_scope {
    ($t:ty) => {
        paste::paste! {
            impl NamespaceScopedResource for $t {}

            #[derive(Debug, derive_more::Deref, derive_more::From)]
            #[cfg(feature = "config")]
            pub struct [<$t Wrapper>]<'a>(&'a $t);

            #[derive(Debug, Default, derive_more::Deref, derive_more::DerefMut)]
            pub struct [<$t Collection>](NamespaceScopedResourceCollection<[<$t Ref>], $t>);

            impl ResourceCollection<[<$t Ref>], $t> for [<$t Collection>] {
                fn insert(&self, resource: $t) {
                    self.0.insert(resource);
                }

                fn remove_by_uid(&self, uid: &str) -> Option<std::sync::Arc<$t>> {
                    self.0.remove_by_uid(uid)
                }

                fn remove_by_ref(&self, ref_: &[<$t Ref>]) -> Option<std::sync::Arc<$t>> {
                    self.0.remove_by_ref(ref_)
                }

                fn iter(&self) -> impl Iterator<Item = ([<$t Ref>], String, std::sync::Arc<$t>)> {
                    self.0.iter()
                }
            }

            #[derive(Clone, Debug, Hash, PartialEq, Eq, derive_more::Deref, derive_more::From)]
            pub struct [<$t Ref>](NamespaceScopedRef<$t>);

            impl ResourceRef<$t> for [<$t Ref>] {}

            impl [<$t Ref>] {
                pub fn new(namespace: &str, name: &str) -> Self {
                    Self(NamespaceScopedRef::new(namespace, name))
                }
            }

            impl From<&$t> for [<$t Ref>] {
                fn from(r: &$t) -> Self {
                    Self(r.into())
                }
            }

            #[derive(Debug, Default, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
            pub struct [<$t RefCollection>](std::collections::HashSet<[<$t Ref>]>);

            impl std::iter::FromIterator<[<$t Ref>]> for [<$t RefCollection>] {
                fn from_iter<T: IntoIterator<Item = [<$t Ref>]>>(iter: T) -> Self {
                    Self(iter.into_iter().collect())
                }
            }
        }
    };
}
