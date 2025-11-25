use kube_core::Resource;
use vg_kube::resources::{ResourceCollection, ResourceRef};

pub struct ResourcesWatcher<K: ResourceRef<R>, R: Resource, C: ResourceCollection<K, R>>
where
    R::DynamicType: 'static + Default,
{
    resources: C,
    k_marker: std::marker::PhantomData<K>,
    r_marker: std::marker::PhantomData<R>,
}

#[must_use] 
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
