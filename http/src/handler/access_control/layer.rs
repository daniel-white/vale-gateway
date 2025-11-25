use crate::handler::access_control::handler::AccessControlFilterHandler;
use crate::handler::access_control::policy::AccessControlPolicyHandler;
use crate::stage::inbound_request::InboundRequestFilterHandler;
use std::sync::Arc;
use thiserror::Error;
use tower::Layer;
use typed_builder::TypedBuilder;
use vg_config::http::filter::access_control::AccessControlFilter;

#[derive(Debug, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct AccessControlFilterHandlerLayer {
    policy_handler: Arc<AccessControlPolicyHandler>,
}

impl Layer<InboundRequestFilterHandler> for AccessControlFilterHandlerLayer {
    type Service = AccessControlFilterHandler;

    fn layer(&self, inner: InboundRequestFilterHandler) -> Self::Service {
        Self::Service::builder()
            .inner(inner)
            .policy_handler(self.policy_handler.clone())
            .build()
    }
}

#[derive(Debug, Error)]
pub enum AccessControlFilterHandlerLayerError {}

impl TryFrom<&AccessControlFilter> for AccessControlFilterHandlerLayer {
    type Error = AccessControlFilterHandlerLayerError;

    fn try_from(value: &AccessControlFilter) -> Result<Self, Self::Error> {
        let policy_handler: AccessControlPolicyHandler = value.into();

        let layer = Self::builder().policy_handler(Arc::new(policy_handler)).build();

        Ok(layer)
    }
}
