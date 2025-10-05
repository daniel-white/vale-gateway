#[macro_export]
macro_rules! cluster_scope {
    ($t:ty) => {
        paste::paste! {
            impl ClusterScopedResource for $t {}

            #[derive(Debug, derive_more::Deref, derive_more::From)]
            #[allow(dead_code)]
            pub(crate) struct [<$t Wrapper>]<'a>(&'a $t);

            #[derive(Debug, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
            pub struct [<$t Collection>](ClusterScopedResourceCollection<$t>);

            impl Default for [<$t Collection>] {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl [<$t Collection>] {
                pub fn new() -> Self {
                    Self(ClusterScopedResourceCollection::new())
                }
            }

            #[derive(Clone, Debug, Hash, PartialEq, Eq, derive_more::Deref, derive_more::From)]
            pub struct [<$t Ref>](ClusterScopedRef<$t>);

            impl [<$t Ref>] {
                pub fn new(r: &$t) -> Self {
                    Self(ClusterScopedRef::new(r))
                }

                pub fn new_named(name: &str) -> Self {
                    Self(ClusterScopedRef::new_named(name))
                }
            }

            #[derive(Debug, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
            pub struct [<$t RefCollection>](std::collections::HashSet<[<$t Ref>]>);

            impl Default for [<$t RefCollection>] {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl [<$t RefCollection>] {
                pub fn new() -> Self {
                    Self(std::collections::HashSet::new())
                }
            }

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
            #[allow(dead_code)]
            pub(crate) struct [<$t Wrapper>]<'a>(&'a $t);

            #[derive(Debug, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
            pub struct [<$t Collection>](NamespaceScopedResourceCollection<$t>);

            impl Default for [<$t Collection>] {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl [<$t Collection>] {
                pub fn new() -> Self {
                    Self(NamespaceScopedResourceCollection::new())
                }
            }

            #[derive(Clone, Debug, Hash, PartialEq, Eq, derive_more::Deref, derive_more::From)]
            pub struct [<$t Ref>](NamespaceScopedRef<$t>);

            impl [<$t Ref>] {
                pub fn new(r: &$t) -> Self {
                    Self(NamespaceScopedRef::new(r))
                }

                pub fn new_named(namespace: &str, name: &str) -> Self {
                    Self(NamespaceScopedRef::new_named(namespace, name))
                }
            }

            #[derive(Debug, derive_more::Deref, derive_more::DerefMut, derive_more::From)]
            pub struct [<$t RefCollection>](std::collections::HashSet<[<$t Ref>]>);

            impl Default for [<$t RefCollection>] {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl [<$t RefCollection>] {
                pub fn new() -> Self {
                    Self(std::collections::HashSet::new())
                }
            }

            impl std::iter::FromIterator<[<$t Ref>]> for [<$t RefCollection>] {
                fn from_iter<T: IntoIterator<Item = [<$t Ref>]>>(iter: T) -> Self {
                    Self(iter.into_iter().collect())
                }
            }
        }
    };
}
