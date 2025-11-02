//! Vale Gateway HTTP Processing Library
//!
//! This library provides comprehensive HTTP processing capabilities for the Vale Gateway,
//! including filtering, routing, policy enforcement, and request/response transformation.
//!
//! # Architecture
//!
//! The library is organized into several key modules:
//!
//! - [`filter`] - HTTP filtering system with Tower service integration
//! - [`route`] - Request routing and matching capabilities  
//! - [`policy`] - Policy enforcement (timeouts, retries, client addressing)
//! - [`rewriting`] - URI and header rewriting utilities
//! - [`listener`] - HTTP listener configuration and management
//! - [`extensions`] - HTTP request/response extensions
//! - [`header`] - HTTP header utilities
//!
//! # Filter System Quick Start
//!
//! The filter system provides a simplified, Tower-based approach to HTTP filtering:
//!
//! ```rust,no_run
//! use vg_http::filter::{
//!     FilterCollection,
//!     handlers::AccessControlFilterHandler
//! };
//! use vg_config::http::filter::AccessControlFilter;
//! use vg_config::http::policy::client_addrs::{AccessControlEffect, IpRef};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create filter configuration
//! let config = AccessControlFilter::builder()
//!     .effect(AccessControlEffect::Allow)
//!     .clients(vec![IpRef::Addr("192.168.1.1".parse()?)])
//!     .build();
//!
//! // Create and compose filters
//! let handler = AccessControlFilterHandler::try_from(config)?;
//! let collection = FilterCollection::builder()
//!     .add_inbound_request(handler)
//!     .build();
//!
//! // Build services for use
//! let services = collection.build_services();
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! - **Simplified Tower Integration**: Clean, idiomatic use of Tower services and layers
//! - **Type Safety**: Strong typing with stage-specific filter traits
//! - **Performance Optimized**: Zero-cost abstractions and minimal allocations
//! - **Modular Design**: Self-contained filter handlers with localized error types
//! - **Comprehensive Testing**: Built-in test utilities and benchmarking support
//! - **Rich Documentation**: Examples and best practices included

pub mod extensions;
pub mod filter;
pub mod header;
pub mod listener;
pub mod policy;
pub mod rewriting;
pub mod route;

// ============================================================================
// Public API - Filter System
// ============================================================================

/// Filter system re-exports for convenient access
pub mod filters {
    //! Convenient re-exports of the filter system components
    //!
    //! This module provides easy access to the most commonly used filter
    //! system components without needing to navigate the full module hierarchy.

    // Core traits and interfaces
    pub use crate::filter::{
        BackendRequestFilter, BackendResponseFilter, FilterHandler, InboundRequestFilter,
        PreBackendFilter, ResponseGenerationFilter,
    };

    // Type aliases and utilities
    pub use crate::filter::{
        BoxedFuture, FilterRequest, FilterResponse, FilterResult, FilterServiceBuilder,
        RequestParts, ResponseParts,
    };

    // Error handling
    pub use crate::filter::FilterError;

    // Service creation and composition
    pub use crate::filter::{
        BackendRequestLayer, BackendResponseLayer, FilterService, InboundRequestLayer,
        PreBackendLayer, ResponseGenerationLayer,
    };
    pub use crate::filter::{FilterServiceFactory, StageServices};

    // Filter collection and builder
    pub use crate::filter::{
        BackendRequestFilterTrait, BackendResponseFilterTrait, FilterCollection,
        InboundRequestFilterTrait, PreBackendFilterTrait, ResponseGenerationFilterTrait,
        StageCounts,
    };

    // Filter handler implementations
    pub use crate::filter::handlers::{
        AccessControlFilterHandler, BackendUriRewriterFilterHandler, HeaderModifierFilterHandler,
        RedirectResponseFilterHandler, StaticResponseFilterHandler,
    };

    // Filter-specific error types (for configuration)
    pub use crate::filter::handlers::access_control::AccessControlError;
    pub use crate::filter::handlers::backend_uri_rewriter::BackendUriRewriterError;
    pub use crate::filter::handlers::header_modifier::HeaderModifierError;
    pub use crate::filter::handlers::redirect_response::RedirectResponseError;
    pub use crate::filter::handlers::static_response::StaticResponseError;

    // Testing utilities (only in test builds)
    #[cfg(test)]
    pub use crate::filter::utils::test_utils;
}

pub use filter::{
    FilterCollection, FilterError, FilterHandler, FilterRequest, FilterResponse, FilterResult,
    FilterServiceFactory,
};

// Filter handlers for direct access
pub use filter::handlers::{
    AccessControlFilterHandler, BackendUriRewriterFilterHandler, HeaderModifierFilterHandler,
    RedirectResponseFilterHandler, StaticResponseFilterHandler,
};
