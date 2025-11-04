use thiserror::Error;
use tower::Service;
use tower::util::BoxCloneService;

mod finalizer;

trait ResponseFilterTrait: Service<http::response::Parts> {}

#[derive(Debug, Error)]
#[error("Backend request filter error")]
pub struct ResponseFilterError;

impl<T> ResponseFilterTrait for T where
    T: Service<http::response::Parts, Response = (), Error = ResponseFilterError>
{
}

pub type ResponseFilter = BoxCloneService<http::response::Parts, (), ResponseFilterError>;
