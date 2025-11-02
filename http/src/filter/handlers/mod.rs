//! Filter handler implementations
//!
//! This module contains all the concrete filter handler implementations.
//! Each handler implements the FilterHandler trait and provides specific
//! filtering functionality for HTTP requests and responses.

pub mod access_control;
pub mod backend_uri_rewriter;
pub mod client_addr;
pub mod header_modifier;
pub mod redirect_response;
pub mod static_response;

pub use backend_uri_rewriter::BackendUriRewriterFilterHandler;
pub use header_modifier::HeaderModifierFilterHandler;
pub use redirect_response::RedirectResponseFilterHandler;
pub use static_response::StaticResponseFilterHandler;
