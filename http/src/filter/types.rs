use crate::filter::error::FilterError;
use std::future::Future;

// Common type aliases to reduce generic complexity
// These aliases provide simpler names for commonly used complex types

/// Standard result type for filter operations
///
/// This provides a consistent return type across all filter operations,
/// reducing the need to specify the full Result<T, FilterError> everywhere.
pub type FilterResult<T> = Result<T, FilterError>;

/// Boxed future type for async operations
///
/// Used when we need to box futures for storage or type erasure.
/// This is the stable alternative to impl Trait in type aliases.
pub type BoxedFuture<T> = std::pin::Pin<Box<dyn Future<Output = T> + Send>>;

/// Boxed service type for type erasure when needed
///
/// This provides a way to store different filter services in collections
/// while maintaining the same interface. Uses Tower's BoxService for proper type erasure.
pub type BoxedFilterService =
    tower::util::BoxService<http::Request<()>, http::Response<()>, FilterError>;

/// Request type used throughout the filter system
///
/// Standardizes on http::Request<()> for filter processing,
/// as filters typically work with request parts and extensions
/// rather than body content.
pub type FilterRequest = http::Request<()>;

/// Response type used throughout the filter system
///
/// Standardizes on http::Response<()> for filter processing,
/// as most filters generate responses without body content
/// or work with response parts.
pub type FilterResponse = http::Response<()>;

/// Request parts type for filters that only need headers and metadata
///
/// Many filters only need to examine or modify request headers
/// and metadata, not the body content.
pub type RequestParts = http::request::Parts;

/// Response parts type for filters that only need headers and metadata
///
/// Similar to RequestParts, many filters only work with response
/// headers and status codes.
pub type ResponseParts = http::response::Parts;

/// Service builder type for composing filter chains
///
/// Provides a convenient alias for the Tower ServiceBuilder
/// configured for filter composition.
pub type FilterServiceBuilder<L> = tower::ServiceBuilder<L>;
