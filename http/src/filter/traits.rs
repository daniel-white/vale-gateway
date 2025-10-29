use tower::Service;

/// Base trait that all filter handlers implement
///
/// This trait extends Tower's Service trait to provide a unified interface
/// for all HTTP filter operations. Unlike the previous implementation,
/// this trait does not require Clone, making it easier to work with
/// stateful filters that use Arc for data sharing.
pub trait FilterHandler: Service<http::Request<()>> + Send + Sync + 'static {
    /// Configuration type used to create this filter handler
    type Config;

    /// Error type specific to this filter handler's configuration/validation
    type ConfigError: std::error::Error + Send + Sync + 'static;

    /// Create a filter handler from configuration
    ///
    /// This method provides a consistent pattern for creating filter handlers
    /// from configuration. Each filter handler must implement this method.
    fn try_from_config(config: Self::Config) -> Result<Self, Self::ConfigError>
    where
        Self: Sized;
}

// Stage-specific marker traits for type safety
// These traits provide compile-time guarantees about which filters
// can be used at which stages of request processing

/// Marker trait for filters that process incoming requests before routing
///
/// Examples: authentication, rate limiting, access control
pub trait InboundRequestFilter: FilterHandler {}

/// Marker trait for filters that process requests after routing but before backend
///
/// Examples: header modification, request transformation
pub trait PreBackendFilter: FilterHandler {}

/// Marker trait for filters that process requests just before sending to backend
///
/// Examples: final URI rewriting, backend-specific headers
pub trait BackendRequestFilter: FilterHandler {}

/// Marker trait for filters that process responses from backend
///
/// Examples: response header modification, response transformation
pub trait BackendResponseFilter: FilterHandler {}

/// Marker trait for filters that generate responses without backend interaction
///
/// Examples: static responses, redirects, error pages
pub trait ResponseGenerationFilter: FilterHandler {}
