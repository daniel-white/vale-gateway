pub mod policy;

use std::sync::Arc;
use crate::filter::handlers::access_control::policy::{
    AccessControlPolicyHandler, EvaluationResult,
};
use crate::filter::handlers::client_addr::extensions::TrustedClientIpAddr;
use crate::policy::error_response::ErrorResponsePolicyHandler;
use crate::policy::error_response::error_codes::ErrorResponseCode;
use http::request::Parts;
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use tower::{Layer, Service};
use typed_builder::TypedBuilder;
use crate::filter::inbound_request::{DynInboundRequestFilter, InboundRequestFilterError, InboundRequestFilterResponse};

#[derive(Debug, Clone, TypedBuilder)]
pub struct AccessControlFilterHandler {
    inner: DynInboundRequestFilter,
    policy_handler: Arc<AccessControlPolicyHandler>,
    error_generator: Arc<ErrorResponsePolicyHandler>,
}

impl Service<Parts> for AccessControlFilterHandler {
    type Response = InboundRequestFilterResponse;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Parts) -> Self::Future {
        let mut inner = self.inner.clone();
        let error_generator = self.error_generator.clone();
        let policy_handler = self.policy_handler.clone();
        Box::pin(async move {
            match req.extensions.get::<TrustedClientIpAddr>() {
                Some(addr) => {
                    match policy_handler.evaluate(addr) {
                        EvaluationResult::Allowed => inner.call(req).await,
                        EvaluationResult::Denied => {
                            let response = error_generator
                                .generate_response(ErrorResponseCode::AccessDenied);
                            let response = InboundRequestFilterResponse::Respond(response);
                            Ok(response)
                        }
                    }
                }
                None => {
                    let response =  error_generator
                        .generate_response(ErrorResponseCode::AccessDenied);
                    let response = InboundRequestFilterResponse::Respond(response);
                    Ok(response)
                }
            }
        })
    }
}

#[derive(Debug, Clone, TypedBuilder)]
pub struct AccessControlFilterHandlerLayer {
    policy_handler: Arc<AccessControlPolicyHandler>,
    error_generator: Arc<ErrorResponsePolicyHandler>,
}

impl Layer<DynInboundRequestFilter> for AccessControlFilterHandlerLayer {
    type Service = AccessControlFilterHandler;

    fn layer(&self, inner: DynInboundRequestFilter) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .policy_handler(self.policy_handler.clone())
            .error_generator(self.error_generator.clone())
            .build()
    }
}

