use std::sync::Arc;
use super::RoutedRequestFilterChain;
use super::finalizer::RoutedRequestFilterChainFinalizer;
use crate::filter::handler::{AccessControlFilterHandlerLayer, AccessControlFilterHandlerLayerError, ErrorResponseHandlerLayer, HeaderModifierFilterHandlerLayer, HeaderModifierFilterHandlerLayerError, RedirectResponseFilterHandlerLayer, RedirectResponseFilterHandlerLayerError, StaticResponseFilterHandlerLayer, StaticResponseFilterHandlerLayerError};
use thiserror::Error;
use tower::{Layer, ServiceExt};
use typed_builder::TypedBuilder;
use vg_config::http::filter::access_control::AccessControlFilter;
use vg_config::http::filter::header_modifier::{HeaderModifierFilter};
use vg_config::http::filter::redirect_response::RedirectResponseFilter;
use vg_config::http::filter::static_response::StaticResponseFilter;
use vg_config::http::route::rule::filter::RequestHeaderModifierRuleFilter;
use crate::policy::error_response::generators::ErrorResponseGenerator;

#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct RoutedRequestFilterChainFactory {
    error_response_generator: Arc<ErrorResponseGenerator>,
    access_control: Vec<AccessControlFilterHandlerLayer>,
    header_modifiers: Vec<HeaderModifierFilterHandlerLayer>,
    static_responses: Vec<StaticResponseFilterHandlerLayer>,
    redirect_responses: Vec<RedirectResponseFilterHandlerLayer>,
}

#[derive(Debug, Error)]
pub enum RoutedRequestFilterChainFactoryError {
    #[error(transparent)]
    AccessControl(#[from] AccessControlFilterHandlerLayerError),
    #[error(transparent)]
    RequestHeaderModifier(#[from] HeaderModifierFilterHandlerLayerError),
    #[error(transparent)]
    StaticResponse(#[from] StaticResponseFilterHandlerLayerError),
    #[error(transparent)]
    RedirectResponse(#[from] RedirectResponseFilterHandlerLayerError),
}

impl RoutedRequestFilterChainFactory {
    pub fn new() -> Self {
        Self::builder()
            .error_response_generator(Arc::new(ErrorResponseGenerator::default()))
            .access_control(Vec::new())
            .header_modifiers(Vec::new())
            .static_responses(Vec::new())
            .redirect_responses(Vec::new())
            .build()
    }

    pub fn error_response_generator(self, error_response_generator: Arc<ErrorResponseGenerator>) -> Self {
        Self::builder()
            .error_response_generator(error_response_generator)
            .access_control(self.access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(self.static_responses)
            .redirect_responses(self.redirect_responses)
            .build()
    }

    pub fn add_access_control(
        self,
        filter: &AccessControlFilter,
    ) -> Result<Self, RoutedRequestFilterChainFactoryError> {
        let layer: AccessControlFilterHandlerLayer = filter.try_into()?;

        let mut access_control = self.access_control;
        access_control.push(layer);

        let builder = Self::builder()
            .error_response_generator(self.error_response_generator)
            .access_control(access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(self.static_responses)
            .redirect_responses(self.redirect_responses)
            .build();

        Ok(builder)
    }

    pub fn add_header_modifier(
        self,
        filter: &RequestHeaderModifierRuleFilter,
    ) -> Result<Self, RoutedRequestFilterChainFactoryError> {
        let filter: &HeaderModifierFilter = filter;
        let layer: HeaderModifierFilterHandlerLayer = filter.try_into()?;

        let mut header_modifiers = self.header_modifiers;
        header_modifiers.push(layer);

        let builder = Self::builder()
            .error_response_generator(self.error_response_generator)
            .access_control(self.access_control)
            .header_modifiers(header_modifiers)
            .static_responses(self.static_responses)
            .redirect_responses(self.redirect_responses)
            .build();

        Ok(builder)
    }

    pub fn add_static_response(
        self,
        filter: &StaticResponseFilter,
    ) -> Result<Self, RoutedRequestFilterChainFactoryError> {
        let layer: StaticResponseFilterHandlerLayer = filter.try_into()?;

        let mut static_responses = self.static_responses;
        static_responses.push(layer);

        let builder = Self::builder()
            .error_response_generator(self.error_response_generator)
            .access_control(self.access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(static_responses)
            .redirect_responses(self.redirect_responses)
            .build();

        Ok(builder)
    }

    pub fn add_redirect_response(
        self,
        filter: &RedirectResponseFilter,
    ) -> Result<Self, RoutedRequestFilterChainFactoryError> {
        let layer: RedirectResponseFilterHandlerLayer = filter.try_into()?;

        let mut redirect_responses = self.redirect_responses;
        redirect_responses.push(layer);

        let builder = Self::builder()
            .error_response_generator(self.error_response_generator)
            .access_control(self.access_control)
            .header_modifiers(self.header_modifiers)
            .static_responses(self.static_responses)
            .redirect_responses(redirect_responses)
            .build();

        Ok(builder)
    }

    pub fn chain(self) -> RoutedRequestFilterChain {
        let mut service = RoutedRequestFilterChainFinalizer::new().boxed_clone();

        service = ErrorResponseHandlerLayer::builder().generator(self.error_response_generator).build().layer(service).boxed_clone();

        for redirect_responses in self.redirect_responses.into_iter().rev() {
            service = redirect_responses.layer(service).boxed_clone();
        }

        for static_response in self.static_responses.into_iter().rev() {
            service = static_response.layer(service).boxed_clone();
        }

        for header_modifier in self.header_modifiers.into_iter().rev() {
            service = header_modifier.layer(service).boxed_clone();
        }

        for access_control in self.access_control.into_iter().rev() {
            service = access_control.layer(service).boxed_clone();
        }

        service.into()
    }
}
