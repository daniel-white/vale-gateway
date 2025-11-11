use std::sync::Arc;
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use tower::Service;
use typed_builder::TypedBuilder;
use crate::filter::stage::backend_request::{BackendRequestFilter, BackendRequestFilterError, BackendRequestFilterResult};
use crate::filter::stage::inbound_request::{InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult};
use crate::filter::stage::response::{ResponseFilter, ResponseFilterError, ResponseFilterResult};
use crate::policy::error_response::generators::ErrorResponseGenerator;

#[derive(Debug, Clone, TypedBuilder)]
pub  struct ErrorResponseFilterHandler<S: Clone> {
    inner: S,
    generator: Arc<ErrorResponseGenerator>
}

impl Service<http::request::Parts> for ErrorResponseFilterHandler<InboundRequestFilterHandler> {
    type Response = InboundRequestFilterResult;
    type Error = InboundRequestFilterError;
    
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::request::Parts) -> Self::Future {
        let mut inner = self.inner.clone();
        let generator = self.generator.clone();
        Box::pin(async move {
            match inner.call(req).await {
                Ok(InboundRequestFilterResult::ErrorResponse(code)) => {
                    let error_response = generator.generate_response(code);
                    Ok(error_response.into())
                }
                res => res
            }
        })
    }
}

impl Service<http::request::Parts> for ErrorResponseFilterHandler<BackendRequestFilter> {
    type Response = BackendRequestFilterResult;
    type Error = BackendRequestFilterError;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::request::Parts) -> Self::Future {
        let mut inner = self.inner.clone();
        let generator = self.generator.clone();
        Box::pin(async move {
            match inner.call(req).await {
                Ok(BackendRequestFilterResult::ErrorResponse(code)) => {
                    let error_response = generator.generate_response(code);
                    Ok(error_response.into())
                }
                res => res
            }
        })
    }
}

impl Service<http::response::Parts> for ErrorResponseFilterHandler<ResponseFilter> {
    type Response = ResponseFilterResult;
    type Error = ResponseFilterError;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, res: http::response::Parts) -> Self::Future {
        let mut inner = self.inner.clone();
        let generator = self.generator.clone();
        Box::pin(async move {
            match inner.call(res).await {
                Ok(ResponseFilterResult::ErrorResponse(code)) => {
                    let error_response = generator.generate_response(code);
                    Ok(error_response.into())
                }
                res => res
            }
        })
    }
}
