//! Filter handler implementations
//!
//! This module contains all the concrete filter handler implementations.
//! Each handler implements the FilterHandler trait and provides specific
//! filtering functionality for HTTP requests and responses.

pub mod access_control;
pub mod backend_uri_rewriter;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;

// Re-export all filter handlers for convenience
pub use access_control::AccessControlFilterHandler;
pub use backend_uri_rewriter::BackendUriRewriterFilterHandler;
pub use header_modifier::HeaderModifierFilterHandler;
pub use redirect_response::RedirectResponseFilterHandler;
pub use static_response::StaticResponseFilterHandler;

// Re-export filter-specific error types for configuration
pub use access_control::AccessControlError;
pub use backend_uri_rewriter::BackendUriRewriterError;
pub use header_modifier::HeaderModifierError;
pub use redirect_response::RedirectResponseError;
pub use static_response::StaticResponseError;
