use tower::Layer;

// Layer traits (for creating services)

/// Filter layers that create services to process incoming request parts before routing (e.g., authentication, rate limiting)
pub trait InboundRequestFilterLayer<S>: Layer<S> + Send + Sync + 'static
where
    S: tower::Service<http::request::Parts> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
}

/// Filter layers that create services to process request parts after routing but before sending to backend (e.g., header modification, URI rewriting)
pub trait PreBackendFilterLayer<S>: Layer<S> + Send + Sync + 'static
where
    S: tower::Service<http::request::Parts> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
}

/// Filter layers that create services to process request parts just before sending to backend (final request modifications)
pub trait BackendRequestFilterLayer<S>: Layer<S> + Send + Sync + 'static
where
    S: tower::Service<http::request::Parts> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
}

/// Filter layers that create services to process response parts from backend before sending to client (e.g., response header modification)
pub trait BackendResponseFilterLayer<S>: Layer<S> + Send + Sync + 'static
where
    S: tower::Service<http::response::Parts> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
}

/// Filter layers that create services to generate responses without backend interaction (e.g., static responses, redirects)
pub trait ResponseGenerationFilterLayer<S>: Layer<S> + Send + Sync + 'static
where
    S: tower::Service<http::request::Parts, Response = http::response::Parts>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
}

// Service traits (for processing requests)

/// Filter handlers that process incoming request parts before routing (e.g., authentication, rate limiting)
pub trait InboundRequestFilterHandler: Send + Sync + 'static {}

/// Filter handlers that process request parts after routing but before sending to backend (e.g., header modification, URI rewriting)
pub trait PreBackendFilterHandler: Send + Sync + 'static {}

/// Filter handlers that process request parts just before sending to backend (final request modifications)
pub trait BackendRequestFilterHandler: Send + Sync + 'static {}

/// Filter handlers that process response parts from backend before sending to client (e.g., response header modification)
pub trait BackendResponseFilterHandler: Send + Sync + 'static {}

/// Filter handlers that generate responses without backend interaction (e.g., static responses, redirects)
pub trait ResponseGenerationFilterHandler: Send + Sync + 'static {}
