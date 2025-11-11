use crate::handler::{
    BackendUriRewriterFilterHandlerLayer, BackendUriRewriterFilterHandlerLayerError,
    ErrorResponseHandlerLayer, ErrorResponseHandlerLayerError, HeaderModifierFilterHandlerLayer,
    HeaderModifierFilterHandlerLayerError,
};
use crate::stage::backend_request::finalizer::BackendRequestFilterChainFinalizer;
use crate::stage::backend_request::{
    BackendRequestFilter, BackendRequestFilterChain, BackendRequestFilterError, factory,
};
use std::sync::Arc;
use thiserror::Error;
use tower::util::BoxCloneServiceLayer;
use tower::{Layer, ServiceExt};
use typed_builder::TypedBuilder;
use vg_config::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;
use vg_config::http::policy::error_response::ErrorResponsePolicy;
use vg_config::http::route::rule::filter::RequestHeaderModifierRuleFilter;

type BackendRequestFilterLayer =
    BoxCloneServiceLayer<BackendRequestFilter, http::request::Parts, (), BackendRequestFilterError>;

#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct BackendRequestFilterChainFactory {
    error_response: ErrorResponseHandlerLayer,
    uri_rewriters: Vec<BackendUriRewriterFilterHandlerLayer>,
    header_modifiers: Vec<HeaderModifierFilterHandlerLayer>,
}

#[derive(Debug, Error)]
pub enum BackendRequestFilterChainFactoryError {
    #[error(transparent)]
    ErrorResponse(#[from] ErrorResponseHandlerLayerError),
    #[error(transparent)]
    UriRewriter(#[from] BackendUriRewriterFilterHandlerLayerError),
    #[error(transparent)]
    HeaderModifier(#[from] HeaderModifierFilterHandlerLayerError),
}

impl BackendRequestFilterChainFactory {
    pub fn new() -> Self {
        Self::builder()
            .error_response(Default::default())
            .uri_rewriters(Vec::new())
            .header_modifiers(Vec::new())
            .build()
    }

    pub fn error_response(
        self,
        policy: &ErrorResponsePolicy,
    ) -> Result<Self, BackendRequestFilterChainFactoryError> {
        let layer: ErrorResponseHandlerLayer = policy.try_into()?;
        let factory = Self::builder()
            .error_response(layer)
            .uri_rewriters(self.uri_rewriters)
            .header_modifiers(self.header_modifiers)
            .build();

        Ok(factory)
    }

    pub fn add_uri_rewriter(
        self,
        filter: &BackendUriRewriterFilter,
    ) -> Result<Self, BackendRequestFilterChainFactoryError> {
        let layer: BackendUriRewriterFilterHandlerLayer = filter.try_into()?;

        let mut uri_rewriters = self.uri_rewriters;
        uri_rewriters.push(layer);

        let factory = Self::builder()
            .error_response(self.error_response)
            .uri_rewriters(uri_rewriters)
            .header_modifiers(self.header_modifiers)
            .build();

        Ok(factory)
    }

    pub fn add_header_modifier(
        self,
        filter: &RequestHeaderModifierRuleFilter,
    ) -> Result<Self, BackendRequestFilterChainFactoryError> {
        let filter: &HeaderModifierFilter = filter;
        let layer: HeaderModifierFilterHandlerLayer = filter.try_into()?;

        let mut header_modifiers = self.header_modifiers;
        header_modifiers.push(layer);

        let factory = Self::builder()
            .error_response(self.error_response)
            .uri_rewriters(self.uri_rewriters)
            .header_modifiers(header_modifiers)
            .build();

        Ok(factory)
    }
}

impl From<BackendRequestFilterChainFactory> for BackendRequestFilterChain {
    fn from(value: BackendRequestFilterChainFactory) -> Self {
        let mut service = BackendRequestFilterChainFinalizer::new().boxed_clone();

        service = value.error_response.layer(service).boxed_clone();

        for uri_rewriter in value.uri_rewriters.into_iter().rev() {
            service = uri_rewriter.layer(service).boxed_clone();
        }

        for header_modifier in value.header_modifiers.into_iter().rev() {
            service = header_modifier.layer(service).boxed_clone();
        }

        service.into()
    }
}
