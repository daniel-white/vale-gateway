# Design Document

## Overview

This design transforms the existing RPC client into a robust, production-ready client using Tower middleware for composable reliability patterns. The solution leverages the Tower ecosystem to provide timeouts, circuit breakers, retries, and reconnection handling while maintaining API compatibility.

The design follows the middleware pattern where each reliability feature is implemented as a separate Tower layer that can be composed together. This approach provides flexibility, testability, and follows Rust ecosystem best practices.

## Architecture

### High-Level Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Consumer      │───▶│ Configuration    │───▶│ Enhanced        │
│   (Gateway)     │    │ Client (API)     │    │ WsClientBuilder │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                         │
                                                         ▼
                       ┌─────────────────────────────────────────┐
                       │      WsClientBuilder with Layers       │
                       │  ┌─────────────────────────────────────┐│
                       │  │      Instrumentation Layer          ││
                       │  └─────────────────────────────────────┘│
                       │  ┌─────────────────────────────────────┐│
                       │  │         Timeout Layer              ││
                       │  └─────────────────────────────────────┘│
                       │  ┌─────────────────────────────────────┐│
                       │  │       Circuit Breaker Layer        ││
                       │  └─────────────────────────────────────┘│
                       │  ┌─────────────────────────────────────┐│
                       │  │         Retry Layer                 ││
                       │  └─────────────────────────────────────┘│
                       │  ┌─────────────────────────────────────┐│
                       │  │      Reconnection Layer             ││
                       │  └─────────────────────────────────────┘│
                       └─────────────────────────────────────────┘
                                         │
                                         ▼
                       ┌─────────────────────────────────────────┐
                       │        Base WebSocket Client            │
                       │         (jsonrpsee WsClient)            │
                       └─────────────────────────────────────────┘
```

### Component Interaction

The layers are applied to the WsClientBuilder before the client is created. Each layer wraps the underlying service and can:
- Intercept and modify requests/responses
- Handle errors and implement fallback behavior
- Emit metrics and traces
- Short-circuit the request flow
- Manage connection lifecycle and reconnection

The layers integrate with jsonrpsee's existing architecture by wrapping the transport layer, allowing us to leverage jsonrpsee's built-in features while adding robustness.

## Components and Interfaces

### 1. Enhanced Transport Builder

The transport builder is enhanced to support Tower layers while maintaining the existing API:

```rust
#[derive(Clone, TypedBuilder)]
pub struct ConfigurationTransport {
    listener_ref: ListenerRef,
    client: Arc<dyn ConfigurationApiClient>,
}

pub struct EnhancedWsClientBuilder {
    builder: WsClientBuilder,
    layers: Vec<Box<dyn Layer<WsClient>>>,
    robust_config: Option<RobustClientConfig>,
}

impl EnhancedWsClientBuilder {
    pub fn new() -> Self;
    pub fn with_robust_config(mut self, config: RobustClientConfig) -> Self;
    pub async fn build(self, uri: String) -> Result<WsClient, ConfigurationClientInitError>;
}
```

### 2. Configuration Client (Public API)

The main client interface remains unchanged to maintain backward compatibility:

```rust
pub struct ConfigurationClient {
    transport: ConfigurationTransport,
}

impl ConfigurationClient {
    pub async fn listener(&self) -> Result<Arc<Listener>, ConfigurationClientError>;
    pub async fn route(&self, route_ref: &RouteRef) -> Result<Arc<Route>, ConfigurationClientError>;
    pub async fn backend(&self, backend_ref: &BackendRef) -> Result<Arc<Backend>, ConfigurationClientError>;
    pub async fn shared_filter(&self, filter_ref: &SharedFilterRef) -> Result<Arc<SharedFilter>, ConfigurationClientError>;
}
```

### 3. Layer Integration with jsonrpsee

The layers integrate with jsonrpsee by wrapping the transport and client creation:

```rust
// Trait for layers that can be applied to WsClientBuilder
pub trait WsClientLayer: Send + Sync {
    fn layer(&self, client: WsClient) -> Box<dyn ConfigurationApiClient>;
}

// Wrapper that implements ConfigurationApiClient with middleware
pub struct LayeredClient {
    inner: WsClient,
    layers: Vec<Box<dyn WsClientLayer>>,
}

impl ConfigurationApiClient for LayeredClient {
    // Delegate to inner client through layer stack
}
```

### 4. Middleware Layers

#### Instrumentation Layer
```rust
use opentelemetry::trace::{Tracer, SpanKind};
use opentelemetry::metrics::Meter;

pub struct InstrumentationLayer {
    tracer: Arc<dyn Tracer + Send + Sync>,
    metrics: Arc<ClientMetrics>,
}

pub struct InstrumentationService<S> {
    inner: S,
    tracer: Arc<dyn Tracer + Send + Sync>,
    metrics: Arc<ClientMetrics>,
}
```

#### Timeout Layer
```rust
use tower::timeout::{Timeout, TimeoutLayer as TowerTimeoutLayer};

pub struct TimeoutLayer {
    timeout: Duration,
}

pub type TimeoutService<S> = Timeout<S>;
```

#### Circuit Breaker Layer
```rust
use failsafe::{CircuitBreaker, Config as CircuitBreakerConfig};

pub struct CircuitBreakerLayer {
    circuit_breaker: Arc<CircuitBreaker<ConfigurationClientError>>,
}

pub struct CircuitBreakerService<S> {
    inner: S,
    circuit_breaker: Arc<CircuitBreaker<ConfigurationClientError>>,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub minimum_throughput: u32,
}
```

#### Retry Layer
```rust
use tower_retry::{Policy, Retry};

pub struct RetryLayer<P> {
    policy: P,
}

pub type RetryService<S, P> = Retry<P, S>;

#[derive(Debug, Clone)]
pub struct ExponentialBackoffPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl<Req> Policy<Req, ConfigurationClientError> for ExponentialBackoffPolicy {
    type Future = Ready<Self>;
    
    fn retry(&self, req: &Req, result: Result<&ConfigurationResponse, &ConfigurationClientError>) -> Option<Self::Future>;
    fn clone_request(&self, req: &Req) -> Option<Req>;
}
```

#### Reconnection Layer
```rust
pub struct ReconnectionLayer {
    config: ReconnectionConfig,
}

pub struct ReconnectionService<S> {
    inner: S,
    connection_manager: Arc<ConnectionManager>,
    config: ReconnectionConfig,
}

#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    pub enable_lazy_connection: bool,
    pub max_reconnect_attempts: Option<u32>,
    pub reconnect_base_delay: Duration,
    pub reconnect_max_delay: Duration,
}
```

### 5. Connection Manager

Handles WebSocket connection lifecycle:

```rust
pub struct ConnectionManager {
    state: Arc<RwLock<ConnectionState>>,
    client_factory: Arc<dyn ClientFactory>,
    config: ReconnectionConfig,
}

#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected(Arc<WsClient>),
    Reconnecting { attempts: u32 },
}

#[async_trait]
pub trait ClientFactory: Send + Sync {
    async fn create_client(&self, uri: &Uri) -> Result<WsClient, ClientError>;
}
```

### 6. Configuration

Centralized configuration for all robustness features:

```rust
#[derive(Debug, Clone, TypedBuilder)]
pub struct RobustClientConfig {
    #[builder(default)]
    pub timeout: Option<TimeoutConfig>,
    #[builder(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    #[builder(default)]
    pub retry: Option<RetryPolicy>,
    #[builder(default)]
    pub reconnection: Option<ReconnectionConfig>,
    #[builder(default)]
    pub instrumentation: InstrumentationConfig,
}

#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub default_timeout: Duration,
    pub per_operation_timeouts: HashMap<String, Duration>,
}
```

## Data Models

### Error Types

Enhanced error types that provide detailed failure information:

```rust
#[derive(Debug, Clone, Error)]
pub enum ConfigurationClientError {
    #[error("Resource not found")]
    NotFound,
    #[error("Request timeout after {timeout:?}")]
    RequestTimeout { timeout: Duration },
    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,
    #[error("Connection unavailable")]
    ConnectionUnavailable,
    #[error("Maximum retries exceeded: {attempts}")]
    MaxRetriesExceeded { attempts: u32 },
    #[error("Service unavailable")]
    ServiceUnavailable,
    #[error("Unknown error: {source}")]
    Unknown { source: Box<dyn std::error::Error + Send + Sync> },
}
```

### Metrics Types

```rust
use opentelemetry::metrics::{Counter, Histogram, Gauge, Meter};

pub struct ClientMetrics {
    pub requests_total: Counter<u64>,
    pub requests_duration: Histogram<f64>,
    pub circuit_breaker_state: Gauge<i64>,
    pub retry_attempts_total: Counter<u64>,
    pub reconnection_attempts_total: Counter<u64>,
    pub active_connections: Gauge<i64>,
}

impl ClientMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            requests_total: meter
                .u64_counter("rpc_client_requests_total")
                .with_description("Total number of RPC requests")
                .init(),
            requests_duration: meter
                .f64_histogram("rpc_client_request_duration_seconds")
                .with_description("Duration of RPC requests in seconds")
                .init(),
            circuit_breaker_state: meter
                .i64_gauge("rpc_client_circuit_breaker_state")
                .with_description("Circuit breaker state (0=closed, 1=open, 2=half-open)")
                .init(),
            retry_attempts_total: meter
                .u64_counter("rpc_client_retry_attempts_total")
                .with_description("Total number of retry attempts")
                .init(),
            reconnection_attempts_total: meter
                .u64_counter("rpc_client_reconnection_attempts_total")
                .with_description("Total number of reconnection attempts")
                .init(),
            active_connections: meter
                .i64_gauge("rpc_client_active_connections")
                .with_description("Number of active connections")
                .init(),
        }
    }
}
```

## Error Handling

### Error Classification

Errors are classified into categories to determine appropriate handling:

1. **Retryable Errors**: Network timeouts, connection errors, temporary service unavailability
2. **Non-Retryable Errors**: Authentication failures, malformed requests, not found errors
3. **Circuit Breaker Errors**: High failure rates, service completely unavailable

### Error Propagation

```rust
impl From<tower::timeout::error::Elapsed> for ConfigurationClientError {
    fn from(err: tower::timeout::error::Elapsed) -> Self {
        ConfigurationClientError::RequestTimeout { 
            timeout: err.duration() 
        }
    }
}

impl From<CircuitBreakerError> for ConfigurationClientError {
    fn from(_: CircuitBreakerError) -> Self {
        ConfigurationClientError::CircuitBreakerOpen
    }
}
```

### Fallback Strategies

- **Circuit Breaker Open**: Return cached data if available, otherwise fail fast
- **Connection Lost**: Queue requests for short period, then fail with appropriate error
- **Retry Exhausted**: Return the last error with retry count information

## Testing Strategy

### Unit Tests

1. **Middleware Layer Tests**: Test each middleware layer in isolation
   - Timeout layer: Verify timeout enforcement and error handling
   - Circuit breaker: Test state transitions and threshold behavior
   - Retry layer: Verify retry logic and backoff behavior
   - Reconnection layer: Test connection management and recovery

2. **Configuration Tests**: Verify configuration parsing and validation

3. **Error Handling Tests**: Test error classification and propagation

### Integration Tests

1. **End-to-End Client Tests**: Test complete client behavior with mock server
2. **Middleware Stack Tests**: Test middleware composition and interaction
3. **Connection Management Tests**: Test reconnection scenarios with network simulation

### Property-Based Tests

1. **Circuit Breaker Properties**: Verify circuit breaker invariants under various failure patterns
2. **Retry Behavior**: Test retry behavior with different error patterns and timing

### Mock Infrastructure

```rust
pub struct MockConfigurationServer {
    behavior: ServerBehavior,
}

pub enum ServerBehavior {
    Normal,
    Slow { delay: Duration },
    Failing { error_rate: f64 },
    Unavailable,
}
```

## Implementation Plan Integration

The implementation will be done incrementally:

1. **Foundation**: Create Tower service abstractions and basic middleware structure
2. **Core Middleware**: Implement timeout, retry, and circuit breaker layers
3. **Connection Management**: Add reconnection handling and connection lifecycle management
4. **Integration**: Wire everything together and update the public API
5. **Testing**: Add comprehensive test coverage
6. **Documentation**: Update documentation and examples

## Dependencies

New dependencies to add to workspace `Cargo.toml`:

```toml
[workspace.dependencies]
# Add these new dependencies
tower = "0.5"
tower-http = "0.6"
tower-retry = "0.3"
tokio-util = "0.7"
pin-project = "1.0"
failsafe = "1.0"
futures-util = "0.3"
```

Update `rpc-client/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies remain...
# Add new workspace dependencies
tower.workspace = true
tower-retry.workspace = true
tokio-util.workspace = true
pin-project.workspace = true
failsafe.workspace = true
futures-util.workspace = true
```

## Backward Compatibility

The public API remains unchanged. Existing code will continue to work without modifications. New robustness features are opt-in through configuration:

```rust
// Existing code continues to work
let client = ConfigurationClient::builder()
    .transport(transport)
    .build();

// New robust configuration is optional
let client = ConfigurationClient::builder()
    .transport(transport)
    .robust_config(RobustClientConfig::builder()
        .timeout(TimeoutConfig::default())
        .circuit_breaker(CircuitBreakerConfig::default())
        .retry(RetryPolicy::default())
        .build())
    .build();
```