use crate::filter::handler::{
    AccessControlFilterHandlerLayer, AccessControlFilterHandlerLayerError,
    ClientAddrFilterHandlerLayer, ClientAddrFilterHandlerLayerError,
    HeaderModifierFilterHandlerLayer, HeaderModifierFilterHandlerLayerError,
};
use crate::filter::stage::inbound_request::finalizer::InboundRequestFilterChainFinalizer;
use crate::filter::stage::inbound_request::{
    EarlyInboundRequestFilterChainFactory, InboundRequestFilterChain, InboundRequestFilterHandler,
};
use thiserror::Error;
use tower::{Layer, ServiceExt};
use vg_config::http::filter::access_control::AccessControlFilter;
use vg_config::http::filter::header_modifier::{HeaderModifierFilter, RequestHeaderModifierFilter};
use vg_config::http::policy::client_addrs::ClientAddrPolicy;

#[derive(Debug, Error)]
pub enum EarlyInboundRequestFilterChainFactoryError {
    #[error(transparent)]
    ClientAddr(#[from] ClientAddrFilterHandlerLayerError),
    #[error(transparent)]
    AccessControl(#[from] AccessControlFilterHandlerLayerError),
    #[error(transparent)]
    RequestHeaderModifier(#[from] HeaderModifierFilterHandlerLayerError),
}

impl EarlyInboundRequestFilterChainFactory {
    pub fn with_client_addr(
        policy: &ClientAddrPolicy,
    ) -> Result<Self, EarlyInboundRequestFilterChainFactoryError> {
        let layer: ClientAddrFilterHandlerLayer = policy.try_into()?;

        let builder = Self::builder().client_addr(layer).build();

        Ok(builder)
    }

    pub fn with_access_control(
        self,
        filter: &AccessControlFilter,
    ) -> Result<Self, EarlyInboundRequestFilterChainFactoryError> {
        let layer: AccessControlFilterHandlerLayer = filter.try_into()?;

        let builder = Self::builder()
            .client_addr(self.client_addr)
            .access_control(Some(layer))
            .header_modifiers(self.header_modifiers)
            .build();

        Ok(builder)
    }

    pub fn add_header_modifier(
        self,
        filter: &RequestHeaderModifierFilter,
    ) -> Result<Self, EarlyInboundRequestFilterChainFactoryError> {
        let filter: &HeaderModifierFilter = filter;
        let layer: HeaderModifierFilterHandlerLayer = filter.try_into()?;

        let mut header_modifiers = self.header_modifiers;
        header_modifiers.push(layer);

        let builder = Self::builder()
            .client_addr(self.client_addr)
            .access_control(self.access_control)
            .header_modifiers(header_modifiers)
            .build();

        Ok(builder)
    }

    pub fn chain(self) -> InboundRequestFilterChain {
        let mut service = InboundRequestFilterChainFinalizer::new().boxed_clone();

        for header_modifier in self.header_modifiers.into_iter().rev() {
            service = header_modifier.layer(service).boxed_clone();
        }

        if let Some(access_control) = self.access_control {
            service = access_control.layer(service).boxed_clone();
        }

        service = self.client_addr.layer(service).boxed_clone();

        service
    }
}
