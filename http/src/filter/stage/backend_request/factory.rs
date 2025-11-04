use crate::filter::stage::backend_request::finalizer::BackendRequestFilterChainFinalizer;
use crate::filter::stage::backend_request::{BackendRequestFilter, BackendRequestFilterError};
use tower::util::BoxCloneServiceLayer;
use tower::{ServiceBuilder, ServiceExt};
use typed_builder::TypedBuilder;

type BackendRequestFilterLayer =
    BoxCloneServiceLayer<BackendRequestFilter, http::request::Parts, (), BackendRequestFilterError>;

#[derive(Debug, TypedBuilder)]
pub struct BackendRequestFilterChainBuilder {
    builder: ServiceBuilder<BackendRequestFilterLayer>,
}

impl BackendRequestFilterChainBuilder {
    pub fn build(self) -> BackendRequestFilter {
        let finalizer: BackendRequestFilter =
            BackendRequestFilterChainFinalizer::new().boxed_clone();
        self.builder.service(finalizer)
    }
}
