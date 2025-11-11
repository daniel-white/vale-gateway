use std::sync::Arc;
use thiserror::Error;
use crate::filter::handler::{BackendUriRewriterFilterHandlerLayer, BackendUriRewriterFilterHandlerLayerError, ErrorResponseHandlerLayer, HeaderModifierFilterHandlerLayer, HeaderModifierFilterHandlerLayerError};
use crate::filter::stage::backend_request::finalizer::BackendRequestFilterChainFinalizer;
use crate::filter::stage::backend_request::{BackendRequestFilter, BackendRequestFilterError, BackendRequestFilterChain};
use tower::util::BoxCloneServiceLayer;
use tower::{Layer, ServiceExt};
use typed_builder::TypedBuilder;
use vg_config::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;
use vg_config::http::route::rule::filter::RequestHeaderModifierRuleFilter;
use crate::policy::error_response::generators::ErrorResponseGenerator;

type BackendRequestFilterLayer =
    BoxCloneServiceLayer<BackendRequestFilter, http::request::Parts, (), BackendRequestFilterError>;

#[derive(Debug, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct BackendRequestFilterChainFactory {
    error_response_generator: Arc<ErrorResponseGenerator>,
    uri_rewriters: Vec<BackendUriRewriterFilterHandlerLayer>,
    header_modifiers: Vec<HeaderModifierFilterHandlerLayer>
}

#[derive(Debug, Error)]
pub enum BackendRequestFilterChainFactoryError {
    #[error(transparent)]
    UriRewriter(#[from]BackendUriRewriterFilterHandlerLayerError),
    #[error(transparent)]
    HeaderModifier(#[from] HeaderModifierFilterHandlerLayerError),
}

impl BackendRequestFilterChainFactory {
    pub fn new() -> Self {
        Self::builder()
            .error_response_generator(Arc::new(ErrorResponseGenerator::default()))
            .uri_rewriters(Vec::new())
            .header_modifiers(Vec::new())
            .build()
    }
    
    pub fn error_response_generator(self, error_response_generator: Arc<ErrorResponseGenerator>) -> Self {
        Self::builder()
            .error_response_generator(error_response_generator)
            .uri_rewriters(self.uri_rewriters)
            .header_modifiers(self.header_modifiers)
            .build()
    }
    
    pub  fn add_uri_rewriter(self, filter: &BackendUriRewriterFilter) -> Result<Self, BackendRequestFilterChainFactoryError> {
        let layer: BackendUriRewriterFilterHandlerLayer = filter.try_into()?;
        
        let mut uri_rewriters = self.uri_rewriters;
        uri_rewriters.push(layer);
        
        let factory = Self::builder()
            .error_response_generator(self.error_response_generator)
            .uri_rewriters(uri_rewriters)
            .header_modifiers(self.header_modifiers)
            .build();
        
        Ok(factory)
    }
    
    pub  fn add_header_modifier(self, filter: &RequestHeaderModifierRuleFilter) -> Result<Self, BackendRequestFilterChainFactoryError> {
        let filter: &HeaderModifierFilter = filter;
        let layer: HeaderModifierFilterHandlerLayer = filter.try_into()?;

        let mut header_modifiers = self.header_modifiers;
        header_modifiers.push(layer);

        let factory = Self::builder()
            .error_response_generator(self.error_response_generator)
            .uri_rewriters(self.uri_rewriters)
            .header_modifiers(header_modifiers)
            .build();

        Ok(factory)
    }
}

impl From<BackendRequestFilterChainFactory> for BackendRequestFilterChain {
    fn from(value: BackendRequestFilterChainFactory) -> Self {
        let mut service = BackendRequestFilterChainFinalizer::new().boxed_clone();
        
        service = ErrorResponseHandlerLayer::builder().generator(value.error_response_generator).build().layer(service).boxed_clone();

        for uri_rewriter in value.uri_rewriters.into_iter().rev() {
            service = uri_rewriter.layer(service).boxed_clone();
        }

        for header_modifier in value.header_modifiers.into_iter().rev() {
            service = header_modifier.layer(service).boxed_clone();
        }
        
        service.into()
    }
}