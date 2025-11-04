use crate::filter::handler::{
    AccessControlFilterHandlerLayer, BackendUriRewriterFilterHandlerLayer,
    HeaderModifierFilterHandlerLayer, RedirectResponseFilterHandlerLayer,
    StaticResponseFilterHandlerLayer,
};
use derive_more::TryUnwrap;

// Filter handler implementations
pub mod handler;

pub mod stage;
// ============================================================================

// Temporary stub for SharedFilterHandler to allow compilation during refactoring
// This will be properly implemented in subsequent tasks
#[derive(Debug, Clone, TryUnwrap)]
pub enum SharedFilterHandlerLayer {
    AccessControl(AccessControlFilterHandlerLayer),
    HeaderModifier(HeaderModifierFilterHandlerLayer),
    BackendUriRewriter(BackendUriRewriterFilterHandlerLayer),
    RedirectResponse(RedirectResponseFilterHandlerLayer),
    StaticResponse(StaticResponseFilterHandlerLayer),
}
