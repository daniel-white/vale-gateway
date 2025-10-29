//! Unified error types for filter runtime operations
//!
//! This module contains only the unified FilterError enum for runtime errors.
//! Filter-specific configuration and validation errors are defined in their
//! respective handler modules to maintain modularity and self-contained modules.

use thiserror::Error;

/// Unified error type for filter runtime operations
///
/// This enum provides a consistent error handling approach for runtime errors
/// across all filters. Configuration and validation errors are defined in their
/// respective filter handler modules and converted to this type when needed.
#[derive(Debug, Error)]
pub enum FilterError {
    /// No client IP found in request extensions
    #[error("No client IP found in request extensions")]
    NoClientIp,

    /// Header modification operation failed
    #[error("Header modification failed: {message}")]
    HeaderModification { message: String },

    /// URI rewriting operation failed
    #[error("URI rewriting failed: {message}")]
    UriRewriting { message: String },

    /// Static response generation failed
    #[error("Static response generation failed: {message}")]
    StaticResponse { message: String },

    /// Access control evaluation failed
    #[error("Access control evaluation failed: {message}")]
    AccessControl { message: String },

    /// Redirect response generation failed
    #[error("Redirect response generation failed: {message}")]
    RedirectResponse { message: String },

    /// Service is not ready to process requests
    #[error("Service not ready")]
    ServiceNotReady,

    /// Inner service returned an error
    #[error("Inner service error")]
    InnerServiceError,

    /// Configuration error during filter creation
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Invalid request format or missing required data
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// HTTP protocol error
    #[error("HTTP protocol error: {message}")]
    HttpProtocol { message: String },
}

// Note: Filter-specific configuration and validation errors are now defined
// in their respective handler modules (e.g., handlers::access_control::AccessControlError)
// and provide automatic conversion to FilterError when needed.
