#[macro_export]
macro_rules! internal_wrapper {
    ($t:ty) => {
        paste::paste! {
            #[derive(Debug, derive_more::Deref, derive_more::From)]
            #[allow(dead_code)]
            pub(crate) struct [<$t Wrapper>]<'a>(&'a $t);
        }
    };
}
