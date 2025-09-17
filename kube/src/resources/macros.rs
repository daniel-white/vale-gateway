#[macro_export]
macro_rules! cluster_scope {
    ($t:ty) => {
        paste! {
            pub type [<$t Collection>] = ClusterScopedResourceCollection<$t>;
            pub type [<$t Ref>]  = ClusterScopedRef<$t>;
        }
    };
}

#[macro_export]
macro_rules! namespace_scope {
    ($t:ty) => {
        paste! {
            pub type [<$t Collection>] = NamespaceScopedResourceCollection<$t>;
            pub type [<$t Ref>]  = NamespaceScopedRef<$t>;
        }
    };
}