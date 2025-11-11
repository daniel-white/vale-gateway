use crate::extensions::TrustedClientIpAddr;
use crate::handler::access_control::policy::{AccessControlPolicyHandler, EvaluationResult};
use crate::handler::error_response::error_code::ErrorResponseCode;
use crate::stage::inbound_request::{
    InboundRequestFilterError, InboundRequestFilterHandler, InboundRequestFilterResult,
};
use futures::future::BoxFuture;
use http::request::Parts;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, TypedBuilder)]
pub struct AccessControlFilterHandler {
    inner: InboundRequestFilterHandler,
    policy_handler: Arc<AccessControlPolicyHandler>,
}

impl Service<Parts> for AccessControlFilterHandler {
    type Response = InboundRequestFilterResult;
    type Error = InboundRequestFilterError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Parts) -> Self::Future {
        let mut inner = self.inner.clone();
        let policy_handler = self.policy_handler.clone();
        Box::pin(async move {
            match req.extensions.get::<TrustedClientIpAddr>() {
                Some(addr) => match policy_handler.evaluate(addr) {
                    EvaluationResult::Allowed => inner.call(req).await,
                    EvaluationResult::Denied => Ok(ErrorResponseCode::AccessDenied.into()),
                },
                None => Ok(ErrorResponseCode::AccessDenied.into()),
            }
        })
    }
}
