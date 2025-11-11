use super::PreRoutingRequestFilterChain;
use super::finalizer::PreRoutingRequestFilterChainFinalizer;
use crate::handler::generator::{ErrorResponseGenerator, ErrorResponseGeneratorConversionError};
use crate::handler::{
    AccessControlFilterHandlerLayer, AccessControlFilterHandlerLayerError,
    ClientAddrFilterHandlerLayer, ClientAddrFilterHandlerLayerError, ErrorResponseHandlerLayer,
    ErrorResponseHandlerLayerError, HeaderModifierFilterHandlerLayer,
    HeaderModifierFilterHandlerLayerError, StaticResponseFilterHandlerLayer,
    StaticResponseFilterHandlerLayerError,
};
use std::sync::Arc;
use thiserror::Error;
use tower::{Layer, ServiceExt};
use typed_builder::TypedBuilder;
use vg_config::http::filter::access_control::AccessControlFilter;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;
use vg_config::http::filter::static_response::StaticResponseFilter;
use vg_config::http::listener::filter::RequestHeaderModifierListenerFilter;
use vg_config::http::policy::client_addrs::ClientAddrPolicy;
use vg_config::http::policy::error_response::ErrorResponsePolicy;

#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct PreRoutingRequestFilterChainFactory {
    client_addr: ClientAddrFilterHandlerLayer,
    error_response: ErrorResponseHandlerLayer,
    access_control: Vec<AccessControlFilterHandlerLayer>,
    header_modifiers: Vec<HeaderModifierFilterHandlerLayer>,
    static_responses: Vec<StaticResponseFilterHandlerLayer>,
}

#[derive(Debug, Error)]
pub enum PreRoutingRequestFilterChainFactoryError {
    #[error(transparent)]
    ErrorResponse(#[from] ErrorResponseHandlerLayerError),
    #[error(transparent)]
    ClientAddr(#[from] ClientAddrFilterHandlerLayerError),
    #[error(transparent)]
    AccessControl(#[from] AccessControlFilterHandlerLayerError),
    #[error(transparent)]
    HeaderModifier(#[from] HeaderModifierFilterHandlerLayerError),
    #[error(transparent)]
    StaticResponse(#[from] StaticResponseFilterHandlerLayerError),
}

impl PreRoutingRequestFilterChainFactory {
    pub fn with_client_addr(
        policy: &ClientAddrPolicy,
    ) -> Result<Self, PreRoutingRequestFilterChainFactoryError> {
        let layer: ClientAddrFilterHandlerLayer = policy.try_into()?;

        let factory = Self::builder()
            .client_addr(layer)
            .error_response(Default::default())
            .access_control(Vec::new())
            .header_modifiers(Vec::new())
            .static_responses(Vec::new())
            .build();

        Ok(factory)
    }

    pub fn error_responses(
        self,
        policy: &ErrorResponsePolicy,
    ) -> Result<Self, PreRoutingRequestFilterChainFactoryError> {
        let layer: ErrorResponseHandlerLayer = policy.try_into()?;

        let factory = Self::builder()
            .client_addr(self.client_addr)
            .error_response(layer)
            .access_control(self.access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(self.static_responses)
            .build();

        Ok(factory)
    }

    pub fn add_access_control(
        self,
        filter: &AccessControlFilter,
    ) -> Result<Self, PreRoutingRequestFilterChainFactoryError> {
        let layer: AccessControlFilterHandlerLayer = filter.try_into()?;

        let mut access_control = self.access_control;
        access_control.push(layer);

        let factory = Self::builder()
            .client_addr(self.client_addr)
            .error_response(self.error_response)
            .access_control(access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(self.static_responses)
            .build();

        Ok(factory)
    }

    pub fn add_header_modifier(
        self,
        filter: &RequestHeaderModifierListenerFilter,
    ) -> Result<Self, PreRoutingRequestFilterChainFactoryError> {
        let filter: &HeaderModifierFilter = filter;
        let layer: HeaderModifierFilterHandlerLayer = filter.try_into()?;

        let mut header_modifiers = self.header_modifiers;
        header_modifiers.push(layer);

        let factory = Self::builder()
            .client_addr(self.client_addr)
            .error_response(self.error_response)
            .access_control(self.access_control)
            .header_modifiers(header_modifiers)
            .static_responses(self.static_responses)
            .build();

        Ok(factory)
    }

    pub fn add_static_response(
        self,
        filter: &StaticResponseFilter,
    ) -> Result<Self, PreRoutingRequestFilterChainFactoryError> {
        let layer: StaticResponseFilterHandlerLayer = filter.try_into()?;

        let mut static_responses = self.static_responses;
        static_responses.push(layer);

        let factory = Self::builder()
            .client_addr(self.client_addr)
            .error_response(self.error_response)
            .access_control(self.access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(static_responses)
            .build();

        Ok(factory)
    }
}

impl From<PreRoutingRequestFilterChainFactory> for PreRoutingRequestFilterChain {
    fn from(value: PreRoutingRequestFilterChainFactory) -> Self {
        let mut service = PreRoutingRequestFilterChainFinalizer::new().boxed_clone();

        service = value.error_response.layer(service).boxed_clone();

        for static_response in value.static_responses.into_iter().rev() {
            service = static_response.layer(service).boxed_clone();
        }

        for header_modifier in value.header_modifiers.into_iter().rev() {
            service = header_modifier.layer(service).boxed_clone();
        }

        for access_control in value.access_control.into_iter().rev() {
            service = access_control.layer(service).boxed_clone();
        }

        service = value.client_addr.layer(service).boxed_clone();

        service.into()
    }
}
