use bytes::Bytes;
use derive_more::From;
use thiserror::Error;
use tower::Service;
use tower::util::BoxCloneService;
use crate::policy::error_response::error_codes::ErrorResponseCode;

mod finalizer;

trait ResponseFilterTrait: Service<http::response::Parts> {}

#[derive(Debug, Error)]
#[error("Backend request filter error_response")]
pub struct ResponseFilterError;

#[derive(Debug, From)]
pub enum ResponseFilterResult {
    Continue(http::response::Parts),
    ErrorResponse(ErrorResponseCode),
    Respond(http::Response<Option<Bytes>>),
}

impl<T> ResponseFilterTrait for T where
    T: Service<http::response::Parts, Response = ResponseFilterResult, Error = ResponseFilterError>
{
}

pub type ResponseFilter = BoxCloneService<http::response::Parts, ResponseFilterResult, ResponseFilterError>;
