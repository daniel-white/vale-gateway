//! HTTP Filter System
//!
//! This module provides a comprehensive HTTP filtering system built on top of the Tower
//! service ecosystem. It offers simplified patterns for creating, composing, and applying
//! HTTP filters with strong type safety and performance optimization.
//!
//! # Architecture
//!
//! The filter system is organized into several key components:
//!
//! - **Handlers**: Concrete filter implementations for specific functionality
//! - **Traits**: Core abstractions and interfaces for filter behavior
//! - **Types**: Common type aliases and utilities for simplified APIs
//! - **Factory**: Service creation and composition utilities
//! - **Layer**: Tower layer implementations for service composition
//! - **Collection**: Builder patterns for managing multiple filters
//! - **Examples**: Usage examples and best practices
//! - **Utils**: Testing and benchmarking utilities
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use crate::filter::{FilterCollection, handlers::AccessControlFilterHandler};
//! use config::http::filter::AccessControlFilter;
//! use config::http::policy::client_addrs::{AccessControlEffect, IpRef};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a filter configuration
//! let config = AccessControlFilter::builder()
//!     .effect(AccessControlEffect::Allow)
//!     .clients(vec![IpRef::Addr("192.168.1.1".parse()?)])
//!     .build();
//!
//! // Create filter handler
//! let handler = AccessControlFilterHandler::try_from(config)?;
//!
//! // Build filter collection
//! let collection = FilterCollection::builder()
//!     .add_inbound_request(handler)
//!     .build();
//!
//! // Create services
//! let services = collection.build_services();
//! # Ok(())
//! # }
//! ```

// Core infrastructure modules
pub mod error;
pub mod factory;
pub mod layer;
pub mod new_collection;
pub mod traits;
pub mod types;

// Filter handler implementations
pub mod handlers;

// Utility modules
pub mod utils;

// Usage examples (disabled for now due to configuration API changes)
// pub mod examples;

// Legacy modules (maintained for backward compatibility during transition)
pub mod collection;
pub mod handler;
pub mod stage_services;

// ============================================================================
// Public API Exports
// ============================================================================

// Core traits and interfaces
pub use traits::{
    BackendRequestFilter, BackendResponseFilter, FilterHandler, InboundRequestFilter,
    PreBackendFilter, ResponseGenerationFilter,
};

// Type aliases and common types
pub use types::{
    BoxedFuture, FilterRequest, FilterResponse, FilterResult, FilterServiceBuilder, RequestParts,
    ResponseParts,
};

// Error types (unified runtime errors)
pub use error::FilterError;

// Service creation and composition
pub use factory::{FilterServiceFactory, StageServices};
pub use layer::{
    BackendRequestLayer, BackendResponseLayer, FilterService, InboundRequestLayer, PreBackendLayer,
    ResponseGenerationLayer,
};

// Filter collection and builder
pub use new_collection::{
    BackendRequestFilterTrait, BackendResponseFilterTrait, FilterCollection,
    InboundRequestFilterTrait, PreBackendFilterTrait, ResponseGenerationFilterTrait, StageCounts,
};

// Filter handler implementations
pub use handlers::{
    AccessControlFilterHandler, BackendUriRewriterFilterHandler, HeaderModifierFilterHandler,
    RedirectResponseFilterHandler, StaticResponseFilterHandler,
};

// Filter-specific error types (for configuration and validation)
pub use handlers::{
    AccessControlError, BackendUriRewriterError, HeaderModifierError, RedirectResponseError,
    StaticResponseError,
};

// Testing utilities
pub use utils::test_utils;

// ============================================================================
// Legacy API Support (Backward Compatibility)
// ============================================================================
// These exports are maintained during the transition period to avoid breaking
// existing code. They will be deprecated and removed in future versions.

pub use collection::{
    FilterHandlerCollection, FilterHandlerCollectionBuilder, FilterHandlerCollectionError,
};
pub use handler::{
    BackendRequestFilterHandler, BackendRequestFilterLayer, BackendResponseFilterHandler,
    BackendResponseFilterLayer, InboundRequestFilterHandler, InboundRequestFilterLayer,
    PreBackendFilterHandler, PreBackendFilterLayer, ResponseGenerationFilterHandler,
    ResponseGenerationFilterLayer,
};
pub use stage_services::{StageServiceBuilder, StageServiceBuilderError};

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
