# Design Document

## Overview

This design transforms the current RPC client architecture from using separate WebSocket connections for configuration and events clients to a single shared connection per process. The approach maximizes reuse of existing robust transport layers while simplifying connection management and reducing resource usage.

The design leverages existing components from the `core` crate, uses `getset` for clean field access, `typed_builder` for ergonomic construction, and reorganizes the transport module for better maintainability. The `ConfigurationEventsClient` becomes the `EventsHandle` directly, and both clients take an `RpcTransport` parameter instead of having connect methods.

**Key Dependencies:**
- `getset` - For clean getter/setter generation on structs
- `typed_builder` - For ergonomic builder pattern implementation
- `vg_core` - For handles, broadcast channels, and other sync primitives

## Architecture

### Current Architecture (Before)

```
┌─────────────────────────────────────────────────────────────┐
│                    Gateway Process                          │
│                                                             │
│  ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │ ConfigurationClient │    │ ConfigurationEventsClient   │ │
│  │                     │    │                             │ │
│  │ ┌─────────────────┐ │    │ ┌─────────────────────────┐ │ │
│  │ │ConfigTransport 1│ │    │ │ ConfigTransport 2       │ │ │
│  │ │                 │ │    │ │                         │ │ │
│  │ │ ┌─────────────┐ │ │    │ │ ┌─────────────────────┐ │ │ │
│  │ │ │ WsClient 1  │ │ │    │ │ │ WsClient 2          │ │ │ │
│  │ │ │ (Layered)   │ │ │    │ │ │ (Layered)           │ │ │ │
│  │ │ └─────────────┘ │ │    │ │ └─────────────────────┘ │ │ │
│  │ └─────────────────┘ │    │ └─────────────────────────┘ │ │
│  └─────────────────────┘    └─────────────────────────────┘ │
│           │                              │                  │
│           ▼                              ▼                  │
│  ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │   WS Connection 1   │    │   WS Connection 2           │ │
│  │   ws://server:9000  │    │   ws://server:9000          │ │
│  └─────────────────────┘    └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Problems with Current Architecture:**
- Duplicate WebSocket connections to the same server
- Separate monitoring and health checking for each connection
- Duplicated transport layer configuration and management
- Higher resource usage (2x connections, monitoring tasks, etc.)

### Target Architecture (After)

```
┌─────────────────────────────────────────────────────────────┐
│                    Gateway Process                          │
│                                                             │
│  ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │ ConfigurationClient │    │ ConfigurationEventsClient   │ │
│  │                     │    │                             │ │
│  │ ┌─────────────────┐ │    │ ┌─────────────────────────┐ │ │
│  │ │  ClientHandle   │ │    │ │  ClientHandle           │ │ │
│  │ └─────────────────┘ │    │ └─────────────────────────┘ │ │
│  └─────────────────────┘    └─────────────────────────────┘ │
│           │                              │                  │
│           ▼                              ▼                  │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                 RpcTransport                            │ │
│  │                                                         │ │
│  │  ┌─────────────────────────────────────────────────┐   │ │
│  │  │            ConnectionManager                    │   │ │
│  │  │  ┌─────────────────────────────────────────┐   │   │ │
│  │  │  │         LayeredClient                   │   │   │ │
│  │  │  │  (Reused from existing transport)       │   │   │ │
│  │  │  └─────────────────────────────────────────┘   │   │ │
│  │  └─────────────────────────────────────────────────┐   │ │
│  └─────────────────────────────────────────────────────────┘ │
│                           │                                  │
│                           ▼                                  │
│           ┌─────────────────────────────────┐                │
│           │      Single WS Connection      │                │
│           │      ws://server:9000          │                │
│           └─────────────────────────────────┘                │
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. RpcTransport (New Component)

**Source**: New component that wraps and reuses existing `LayeredClient`
**Destination**: Central connection manager for the process

```rust
/// RPC transport instance for the process
/// Reuses existing LayeredClient and robustness features
pub struct RpcTransport {
    /// Reused from existing transport/layers/mod.rs
    client: Arc<LayeredClient>,
    /// Connection configuration (reused from existing ConfigurationTransportOptions)
    config: SharedTransportConfig,
    /// Reused from core/src/sync/handles.rs
    connection_handle: Handle,
    /// Reused from existing transport/layers/monitoring_manager.rs
    monitoring_manager: Arc<tokio::sync::Mutex<MonitoringManager>>,
    /// Event distribution using core broadcast channels
    event_sender: vg_core::sync::broadcast::Sender<ConfigurationEvent>,
}

impl RpcTransport {
    /// Initialize the RPC transport
    /// Reuses existing EnhancedWsClientBuilder and RobustClientConfig
    pub async fn initialize(
        address: Uri,
        robust_config: RobustClientConfig,
    ) -> Result<(), RpcTransportError> {
        // Reuse existing client building logic from builder.rs
        let layered_client = EnhancedWsClientBuilder::new()
            .with_robust_config(robust_config)
            .build(address.to_string())
            .await?;
        
        // Initialize transport instance
        // Implementation details...
    }
}
```

### 2. RpcTransport as Transport Parameter

**Source**: Replaces individual `ConfigurationTransport` instances  
**Destination**: Single transport passed to both client constructors

```rust
/// RPC transport that both clients use
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct RpcTransport {
    /// Reference to shared LayeredClient (from existing transport)
    #[getset(get = "pub")]
    client: Arc<LayeredClient>,
    
    /// Connection configuration (reused from existing)
    #[getset(get = "pub")]
    config: RpcTransportConfig,
    
    /// Connection handle using core utilities
    #[getset(get = "pub")]
    connection_handle: Handle,
    
    /// Event distribution using core broadcast channels
    #[getset(get = "pub")]
    event_sender: vg_core::sync::broadcast::Sender<ConfigurationEvent>,
    
    /// Monitoring manager (reused from existing transport)
    #[getset(get = "pub")]
    monitoring_manager: Arc<tokio::sync::Mutex<MonitoringManager>>,
}

impl RpcTransport {
    /// Create a new RPC transport
    /// Reuses existing EnhancedWsClientBuilder and RobustClientConfig
    pub async fn new(
        address: Uri,
        robust_config: RobustClientConfig,
    ) -> Result<Self, RpcTransportError> {
        // Reuse existing client building logic from builder.rs
        let layered_client = EnhancedWsClientBuilder::new()
            .with_robust_config(robust_config.clone())
            .build(address.to_string())
            .await?;
        
        // Create event distribution channel using core broadcast
        let (event_sender, _) = vg_core::sync::broadcast::channel(1024);
        
        // Create connection handle using core handles
        let (connection_handle, _) = vg_core::sync::handles::handles();
        
        // Create monitoring manager (reused from existing)
        let monitoring_config = robust_config.internal_monitoring.to_monitoring_config();
        let monitoring_manager = Arc::new(tokio::sync::Mutex::new(
            MonitoringManager::new(monitoring_config, address.clone())
        ));
        
        Ok(Self::builder()
            .client(Arc::new(layered_client))
            .config(RpcTransportConfig::builder()
                .robust_config(robust_config)
                .address(address)
                .event_buffer_size(1024)
                .build())
            .connection_handle(connection_handle)
            .event_sender(event_sender)
            .monitoring_manager(monitoring_manager)
            .build())
    }
}
```

### 3. Updated Client APIs (Modified Components)

**Source**: Existing `ConfigurationClient` and `ConfigurationEventsClient` from api.rs and events/client.rs
**Destination**: Updated to take RpcTransport as constructor parameter

```rust
// In api.rs - Updated ConfigurationClient
/// Configuration client that uses RPC transport
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct ConfigurationClient {
    /// RPC transport instance
    #[getset(get = "pub")]
    transport: RpcTransport,
    
    /// Listener reference for this client
    #[getset(get = "pub")]
    listener_ref: ListenerRef,
}

impl ConfigurationClient {
    /// Create a new ConfigurationClient with RPC transport
    /// This replaces the connect() method - transport is now passed in
    pub fn new(transport: RpcTransport, listener_ref: impl Into<ListenerRef>) -> Self {
        Self::builder()
            .transport(transport)
            .listener_ref(listener_ref.into())
            .build()
    }
    
    /// All existing methods remain the same, but use shared transport
    pub async fn listener(&self) -> Result<Arc<Listener>, ConfigurationClientError> {
        // Reuse existing tracing and request logic from api.rs
        let span = TRACER
            .span_builder("ConfigurationClient::listener")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);

        let req = GetListenerRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.listener_ref().clone())
            .build();

        // Use RPC transport client instead of individual transport
        let listener = self.transport().client().listener(req).await.map_err(|err| {
            tracing::error!("Failed to get listener: {:?}", err);
            ConfigurationClientError::from(err)
        })?;

        Ok(Arc::new(listener))
    }
    
    // Other methods (route, backend, shared_filter) follow same pattern...
}

// In events/client.rs - ConfigurationEventsClient becomes the EventsHandle
/// Configuration events client that uses RPC transport
/// This IS the EventsHandle - no separate handle needed
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Getters, TypedBuilder)]
pub struct ConfigurationEventsClient {
    /// RPC transport instance
    #[getset(get = "pub")]
    transport: RpcTransport,
    
    /// Listener reference for this client
    #[getset(get = "pub")]
    listener_ref: ListenerRef,
    
    /// Event receiver using core broadcast channels
    #[getset(get = "pub")]
    #[builder(default_code = "self.transport.event_sender().subscribe()")]
    event_receiver: vg_core::sync::broadcast::Receiver<ConfigurationEvent>,
    
    /// Subscription handle using core handles
    #[getset(get = "pub")]
    #[builder(default)]
    subscription_handle: Option<Handle>,
}

impl ConfigurationEventsClient {
    /// Create a new ConfigurationEventsClient with RPC transport
    /// This replaces the connect() method - transport is now passed in
    pub fn new(transport: RpcTransport, listener_ref: impl Into<ListenerRef>) -> Self {
        let listener_ref = listener_ref.into();
        let event_receiver = transport.event_sender().subscribe();
        
        Self::builder()
            .transport(transport)
            .listener_ref(listener_ref)
            .event_receiver(event_receiver)
            .build()
    }
    
    /// Start event subscription on RPC transport connection
    /// Reuses existing subscription logic from events/client.rs
    pub async fn start(&mut self) -> Result<Handle, ConfigurationEventClientError> {
        // Reuse existing subscription request building
        let span = TRACER
            .span_builder("ConfigurationEventClient::events")
            .with_kind(SpanKind::Client)
            .start(&*TRACER);
            
        let req = SubscribeEventsRequest::builder()
            .context(RequestContext::new(span))
            .listener_ref(self.listener_ref().clone())
            .build();

        let mut subscription = self.transport().client().events(req).await?;
        
        // Use core handles for task management
        let (handle, mut stop_handle) = vg_core::sync::handles::handles();
        
        // Event processing task (reused logic from existing events/client.rs)
        let event_sender = self.transport().event_sender().clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = subscription.next() => {
                        if let Some(Ok(event)) = event {
                            let channel = event.context().propagation_channel();
                            let mut span = TRACER.span_builder("ConfigurationEventClient::recv")
                                .with_kind(SpanKind::Consumer)
                                .start_with_context(&*TRACER, &channel.into());
                            let _ = event_sender.send(event.event());
                            span.end();
                        } else {
                            break;
                        }
                    },
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }
        });
        
        self.subscription_handle = Some(handle.clone());
        Ok(handle)
    }
    
    /// Get events receiver - now returns the internal receiver
    pub fn events(&self) -> ConfigurationEventsReceiver {
        // Create receiver from the RPC transport event distribution
        ConfigurationEventsReceiver::builder()
            .tx(self.transport().event_sender().clone())
            .rx(self.transport().event_sender().subscribe())
            .build()
    }
}
```

## Data Models

### Configuration Reuse

**Source**: Existing `RobustClientConfig`, `ConfigurationTransportOptions`
**Destination**: Reused with minimal modifications for RPC transport, using getset and typed_builder

```rust
/// Reused from existing config.rs with additions for RPC transport
/// Uses getset for clean field access and typed_builder for construction
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct RpcTransportConfig {
    /// Reused existing robust configuration
    #[getset(get = "pub")]
    robust_config: RobustClientConfig,
    
    /// Server address for the connection
    #[getset(get = "pub")]
    address: Uri,
    
    /// Event buffer size for event distribution
    #[getset(get = "pub")]
    #[builder(default = 1024)]
    event_buffer_size: usize,
}

impl From<ConfigurationTransportOptions> for RpcTransportConfig {
    fn from(options: ConfigurationTransportOptions) -> Self {
        Self::builder()
            .robust_config(options.robust_config().cloned().unwrap_or_default())
            .address(options.address().clone())
            .event_buffer_size(1024)
            .build()
    }
}
```

### Error Handling Reuse

**Source**: Existing error types from api.rs and events/client.rs
**Destination**: Reused with minimal additions

```rust
/// Reuse existing error types with additions for RPC transport
#[derive(Debug, Error)]
pub enum RpcTransportError {
    /// Reused from existing ConfigurationClientInitError
    #[error("Transport initialization failed")]
    InitializationFailed(#[from] ConfigurationClientInitError),
    
    /// New error for RPC transport specific issues
    #[error("RPC transport already initialized with different configuration")]
    ConfigurationMismatch,
    
    /// Reused from existing monitoring errors
    #[error("Monitoring error")]
    MonitoringError(#[from] crate::transport::layers::MonitoringError),
}
```

## Transport Module Reorganization

### Current Structure (Source)
```
rpc-client/src/transport/
├── mod.rs (complex ClientWrapper enum and transport logic)
├── layers/
│   ├── mod.rs (layer trait and LayeredClient)
│   ├── circuit_breaker.rs
│   ├── connection.rs
│   ├── connection_logger.rs
│   ├── connection_tests.rs
│   ├── internal_monitor.rs
│   ├── monitoring_manager.rs
│   ├── reconnection.rs
│   ├── retry.rs
│   ├── startup_logger.rs
│   ├── startup_manager.rs
│   ├── status.rs
│   └── timeout.rs
```

### Target Structure (Destination)
```
rpc-client/src/transport/
├── mod.rs (simplified, exports main components)
├── rpc.rs (new RpcTransport implementation)
├── layers/
│   ├── mod.rs (cleaned up layer trait and LayeredClient - reused)
│   ├── robustness/ (new submodule for robustness layers)
│   │   ├── mod.rs
│   │   ├── circuit_breaker.rs (moved from parent)
│   │   ├── retry.rs (moved from parent)
│   │   └── timeout.rs (moved from parent)
│   ├── connection/ (new submodule for connection management)
│   │   ├── mod.rs
│   │   ├── manager.rs (renamed from connection.rs)
│   │   ├── reconnection.rs (moved from parent)
│   │   └── status.rs (moved from parent)
│   └── monitoring/ (new submodule for monitoring)
│       ├── mod.rs
│       ├── internal_monitor.rs (moved from parent)
│       ├── monitoring_manager.rs (moved from parent)
│       ├── connection_logger.rs (moved from parent)
│       ├── startup_logger.rs (moved from parent)
│       └── startup_manager.rs (moved from parent)
```

## Core Utilities Integration

### Using Core Handles

**Source**: `core/src/sync/handles.rs`
**Usage**: Replace custom task management with core handles

```rust
// Replace custom task spawning with core handles
use vg_core::sync::handles::{Handle, handles};

// In RpcTransport - using getset for clean access
impl RpcTransport {
    async fn start_connection_monitoring(&self) -> Handle {
        let (handle, mut stop_handle) = handles();
        
        // Use existing monitoring logic but with core handles
        let monitoring_manager = self.monitoring_manager().clone();
        tokio::spawn(async move {
            // Existing monitoring loop from monitoring_manager.rs
            // but with proper handle-based shutdown
        });
        
        handle
    }
}
```

### Using Core Broadcast Channels

**Source**: `core/src/sync/broadcast/mod.rs`
**Usage**: Replace custom event distribution with core broadcast

```rust
// Replace custom event channels with core broadcast
use vg_core::sync::broadcast::{channel, Sender, Receiver};

// In RpcTransport initialization - using typed_builder
let (event_sender, _) = channel::<ConfigurationEvent>(1024);

// In RpcTransport::builder()
Self::builder()
    .event_sender(event_sender)
    // ... other fields
    .build()
```

### Using Core Synchronization

**Source**: `core/src/sync/arc_watch/mod.rs` and other sync primitives
**Usage**: Use for connection state management with getset and typed_builder

```rust
use vg_core::sync::arc_watch::{ArcWatch, Sender as WatchSender};

// For connection state tracking - using getset and typed_builder
#[derive(Debug, Clone, Getters, TypedBuilder)]
pub struct ConnectionState {
    #[getset(get = "pub")]
    status: ConnectionStatus,
    
    #[getset(get = "pub")]
    last_success: Instant,
    
    #[getset(get = "pub")]
    consecutive_failures: u32,
}

// In RpcTransport - using getset for clean access
let (state_sender, state_receiver) = ArcWatch::new(ConnectionState::builder()
    .status(ConnectionStatus::Connecting)
    .last_success(Instant::now())
    .consecutive_failures(0)
    .build());
```

## Error Handling

### Reused Error Classification

**Source**: Existing `ErrorClassification` trait from api.rs
**Destination**: Extended for shared transport errors

```rust
// Reuse existing error classification logic
impl ErrorClassification for RpcTransportError {
    fn is_retryable(&self) -> bool {
        match self {
            RpcTransportError::InitializationFailed(e) => {
                // Delegate to existing error classification
                match e {
                    ConfigurationClientInitError::WsClientError => true,
                }
            }
            RpcTransportError::ConfigurationMismatch => false,
            RpcTransportError::MonitoringError(_) => true,
        }
    }
    
    // Other methods reuse existing logic...
}
```

## Testing Strategy

### Unit Tests

1. **RpcTransport Tests**: Test connection lifecycle management
2. **Client Tests**: Test ConfigurationClient and ConfigurationEventsClient functionality  
3. **Migration Tests**: Ensure client APIs work with new transport parameter approach
4. **Core Integration Tests**: Test integration with core utilities

### Integration Tests

1. **End-to-End Single Connection**: Test both clients using same RpcTransport
2. **Resilience Tests**: Test RPC transport connection recovery and reconnection
3. **Performance Tests**: Compare resource usage before/after
4. **API Compatibility**: Ensure client construction works with transport parameter

## Migration Strategy

### Phase 1: Create RPC Transport Infrastructure
- Implement RpcTransport using existing LayeredClient
- Add core utilities integration (handles, broadcast channels)
- Add getset and typed_builder dependencies
- Create RpcTransportConfig and error types

### Phase 2: Update Client Implementations
- Modify ConfigurationClient to take RpcTransport parameter
- Modify ConfigurationEventsClient to take RpcTransport parameter
- Remove connect() methods, replace with constructor pattern
- Add getset and typed_builder to client structs

### Phase 3: Reorganize Transport Module
- Move transport layer files to new organized structure
- Update imports and module declarations
- Clean up unused code and duplications
- Update documentation

### Phase 4: Optimize and Clean Up
- Remove unused transport code
- Optimize RPC transport connection performance
- Add comprehensive monitoring for RPC transport
- Performance testing and optimization

## API Changes

The design changes the client construction pattern:

**Before:**
```rust
let client = ConfigurationClient::connect(listener_ref, address).await?;
let events = ConfigurationEventsClient::connect(listener_ref, address).await?;
```

**After:**
```rust
let transport = RpcTransport::new(address, robust_config).await?;
let client = ConfigurationClient::new(transport.clone(), listener_ref);
let events = ConfigurationEventsClient::new(transport, listener_ref);
```

- **Client Methods**: All existing client methods remain unchanged
- **Configuration**: Existing `RobustClientConfig` continues to work
- **Error Types**: All existing error types are preserved
- **Behavior**: Client behavior remains the same, just with shared connection

## Performance Benefits

1. **Resource Usage**: 50% reduction in WebSocket connections
2. **Memory Usage**: Reduced memory footprint from eliminating duplicate transport layers
3. **Connection Management**: Single monitoring task instead of multiple
4. **Network Efficiency**: Better connection utilization and reduced overhead
5. **Startup Time**: Faster startup due to single connection establishment

The design leverages existing robust transport infrastructure while providing these benefits with a cleaner constructor-based API.