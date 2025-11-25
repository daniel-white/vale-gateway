use crate::handler::error_response::generator::ErrorResponseGenerator;
use crate::stage::backend_request::{BackendRequestFilter, BackendRequestFilterError, BackendRequestFilterResult};
use crate::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult,
};
use crate::stage::response::{ResponseFilter, ResponseFilterError, ResponseFilterResult};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct ErrorResponseHandler<S: Clone> {
    inner: S,
    generator: ErrorResponseGenerator,
}

impl Service<http::request::Parts> for ErrorResponseHandler<InboundRequestFilterHandler> {
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
                res => res,
            }
        })
    }
}

impl Service<http::request::Parts> for ErrorResponseHandler<BackendRequestFilter> {
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
                res => res,
            }
        })
    }
}

impl Service<http::response::Parts> for ErrorResponseHandler<ResponseFilter> {
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
                res => res,
            }
        })
    }
}
