// Core infrastructure modules
pub mod error;

// Filter handler implementations
pub mod handlers;
pub  mod inbound_request;

pub mod backend_request;
pub mod response;
// ============================================================================

// Temporary stub for SharedFilterHandler to allow compilation during refactoring
// This will be properly implemented in subsequent tasks
#[derive(Debug, Clone)]
pub enum SharedFilterHandler {
    AccessControl(AccessControlFilterHandler),
    HeaderModifier(HeaderModifierFilterHandler),
    BackendUriRewriter(BackendUriRewriterFilterHandler),
    RedirectResponse(RedirectResponseFilterHandler),
    StaticResponse(StaticResponseFilterHandler),
}

impl SharedFilterHandler {
    /// Attempt to unwrap as AccessControlFilterHandler
    pub fn try_unwrap_access_control(self) -> Result<AccessControlFilterHandler, Self> {
        match self {
            SharedFilterHandler::AccessControl(handler) => Ok(handler),
            other => Err(other),
        }
    }

    /// Attempt to unwrap as HeaderModifierFilterHandler
    pub fn try_unwrap_header_modifier(self) -> Result<HeaderModifierFilterHandler, Self> {
        match self {
            SharedFilterHandler::HeaderModifier(handler) => Ok(handler),
            other => Err(other),
        }
    }

    /// Attempt to unwrap as BackendUriRewriterFilterHandler
    pub fn try_unwrap_backend_uri_rewriter(self) -> Result<BackendUriRewriterFilterHandler, Self> {
        match self {
            SharedFilterHandler::BackendUriRewriter(handler) => Ok(handler),
            other => Err(other),
        }
    }

    /// Attempt to unwrap as RedirectResponseFilterHandler
    pub fn try_unwrap_redirect_response(self) -> Result<RedirectResponseFilterHandler, Self> {
        match self {
            SharedFilterHandler::RedirectResponse(handler) => Ok(handler),
            other => Err(other),
        }
    }

    /// Attempt to unwrap as StaticResponseFilterHandler
    pub fn try_unwrap_static_response(self) -> Result<StaticResponseFilterHandler, Self> {
        match self {
            SharedFilterHandler::StaticResponse(handler) => Ok(handler),
            other => Err(other),
        }
    }
}
