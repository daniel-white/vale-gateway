# Design Document

## Overview

This design simplifies the gateway's main function by moving all transport management responsibilities into the RPC clients themselves. The clients become fully self-managing, handling connection lifecycle, monitoring, and resilience internally. The gateway focuses solely on its core responsibilities: component wiring and coordination.

The approach leverages the existing robust client infrastructure to eliminate the complex monitoring, health checking, and error handling logic currently scattered throughout the gateway's main function.

## Architecture

### Current Architecture Problems

```
┌─────────────────────────────────────────────────────────────┐
│                    Gateway Main Function                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ Client Creation │  │ Connection      │  │ Health      │ │
│  │ & Validation    │  │ Monitoring      │  │ Checking    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ Timeout         │  │ Error Handling  │  │ Status      │ │
│  │ Management      │  │ & Recovery      │  │ Logging     │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │ Component       │  │ Task            │                  │
│  │ Coordination    │  │ Management      │                  │
│  └─────────────────┘  └─────────────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

### Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Gateway Main Function                    │
│  ┌─────────────────┐  ┌─────────────────┐                  │
│  │ Component       │  │ Task            │                  │
│  │ Coordination    │  │ Management      │                  │
│  └─────────────────┘  └─────────────────┘                  │
└─────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                   Self-Managing Clients                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ Connection      │  │ Health          │  │ Error       │ │
│  │ Management      │  │ Monitoring      │  │ Handling    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ Reconnection    │  │ Status          │  │ Timeout     │ │
│  │ Logic           │  │ Reporting       │  │ Management  │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. Enhanced Client Factory Methods

The clients provide simple factory methods that encapsulate all complexity:

```rust
impl ConfigurationClient {
    /// Create a robust, self-managing client with all resilience features enabled
    /// Handles connection management, monitoring, and resilience internally
    pub async fn connect(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Result<Self, ConfigurationClientError> {
        let config = RobustClientConfig::default()
            .with_startup_mode(StartupMode::Graceful)
            .with_internal_monitoring(true)
            .with_comprehensive_logging(true);
        
        Self::connect_with_config(listener_ref, address, config).await
    }
}

impl ConfigurationEventsClient {
    /// Create a robust, self-managing events client
    pub async fn connect(
        listener_ref: impl Into<ListenerRef>,
        address: impl Into<Uri>,
    ) -> Result<Self, ConfigurationClientError> {
        let config = RobustClientConfig::default()
            .with_startup_mode(StartupMode::Graceful)
            .with_event_stream_optimization(true);
        
        Self::connect_with_config(listener_ref, address, config).await
    }
}
```

### 2. Internal Connection Monitoring

The clients handle all monitoring internally without exposing complexity:

```rust
pub struct InternalConnectionMonitor {
    client: Arc<dyn ConfigurationApiClient>,
    config: MonitoringConfig,
    metrics: Arc<ClientMetrics>,
    logger: ConnectionLogger,
}

impl InternalConnectionMonitor {
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run_monitoring_loop().await;
        })
    }
    
    async fn run_monitoring_loop(&self) {
        let mut interval = tokio::time::interval(self.config.check_interval);
        let mut consecutive_failures = 0;
        let mut last_success = Instant::now();
        
        loop {
            interval.tick().await;
            
            match self.perform_health_check().await {
                Ok(response_time) => {
                    if consecutive_failures > 0 {
                        self.logger.log_connection_recovered(response_time, consecutive_failures);
                    }
                    consecutive_failures = 0;
                    last_success = Instant::now();
                }
                Err(e) => {
                    consecutive_failures += 1;
                    let downtime = last_success.elapsed();
                    
                    if consecutive_failures >= self.config.critical_threshold {
                        self.logger.log_critical_connection_failure(downtime, consecutive_failures);
                    } else {
                        self.logger.log_connection_failure(consecutive_failures, &e);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub check_interval: Duration,
    pub health_check_timeout: Duration,
    pub critical_threshold: u32,
    pub enable_heartbeat_logging: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            critical_threshold: 10, // 5 minutes of failures
            enable_heartbeat_logging: false, // Reduce log noise
        }
    }
}
```

### 3. Enhanced Startup Handling

The clients manage their own startup process with comprehensive logging:

```rust
pub struct StartupManager {
    config: StartupConfig,
    logger: StartupLogger,
}

impl StartupManager {
    pub async fn handle_startup<F, Fut>(
        &self,
        connection_factory: F,
    ) -> Result<Arc<dyn ConfigurationApiClient>, ConfigurationClientError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<Arc<dyn ConfigurationApiClient>, ConfigurationClientError>>,
    {
        match self.config.mode {
            StartupMode::Graceful => {
                self.logger.log_startup_begin();
                
                match timeout(self.config.initial_timeout, connection_factory()).await {
                    Ok(Ok(client)) => {
                        self.logger.log_startup_success();
                        Ok(client)
                    }
                    Ok(Err(e)) | Err(_) => {
                        self.logger.log_startup_fallback(&e);
                        // Return a client that will connect in background
                        self.create_background_connecting_client().await
                    }
                }
            }
            StartupMode::Lazy => {
                self.logger.log_lazy_startup();
                self.create_lazy_client().await
            }
            StartupMode::FailFast => {
                connection_factory().await
            }
        }
    }
}

pub struct StartupLogger {
    span: tracing::Span,
}

impl StartupLogger {
    pub fn log_startup_begin(&self) {
        tracing::info!(
            target: "rpc_client::startup",
            "Initializing configuration client with graceful startup mode"
        );
    }
    
    pub fn log_startup_success(&self) {
        tracing::info!(
            target: "rpc_client::startup",
            "✓ Configuration client connected successfully during startup"
        );
    }
    
    pub fn log_startup_fallback(&self, error: &ConfigurationClientError) {
        tracing::info!(
            target: "rpc_client::startup",
            error = %error,
            "Configuration service unavailable during startup - client will connect in background"
        );
    }
    
    pub fn log_lazy_startup(&self) {
        tracing::info!(
            target: "rpc_client::startup",
            "Configuration client created in lazy mode - will connect on first request"
        );
    }
}
```

### 4. Simplified Gateway Main Function

The gateway main function becomes focused and clean:

```rust
#[tokio::main]
async fn main() {
    init("vg-gateway");
    
    if let Err(e) = run_gateway().await {
        tracing::error!("Gateway startup failed: {}", e);
        std::process::exit(1);
    }
}

async fn run_gateway() -> Result<(), Box<dyn Error>> {
    let listener_ref = "example_listener".to_string();
    let server_uri = Uri::from_static("ws://localhost:9000");
    
    // Validate basic parameters
    validate_startup_parameters(&listener_ref, &server_uri)?;
    
    // Create self-managing clients
    let client = ConfigurationClient::connect(listener_ref.clone(), server_uri.clone()).await?;
    let events = ConfigurationEventsClient::connect(listener_ref, server_uri).await?;
    
    tracing::info!("🚀 Gateway clients initialized successfully");
    
    // Create and wire components
    let components = create_gateway_components(client, events).await?;
    
    // Start all components
    let handles = start_gateway_components(components).await?;
    
    tracing::info!("🚀 Gateway startup complete - all components running");
    
    // Wait for any component to stop (which shouldn't happen)
    wait_for_completion(handles).await;
    
    Ok(())
}

fn validate_startup_parameters(listener_ref: &str, server_uri: &Uri) -> Result<(), Box<dyn Error>> {
    if listener_ref.is_empty() {
        return Err("Listener reference cannot be empty".into());
    }
    
    match server_uri.scheme_str() {
        Some("ws") | Some("wss") => Ok(()),
        _ => Err("Invalid URI scheme: must be 'ws' or 'wss'".into()),
    }
}

struct GatewayComponents {
    source_configuration: SourceConfigurationRegistry,
    shared_filter_handlers: SharedFilterHandlersManager,
    backends_configurator: BackendConfigurator,
    current_location: CurrentLocationConfigurator,
}

async fn create_gateway_components(
    client: ConfigurationClient,
    events: ConfigurationEventsClient,
) -> Result<GatewayComponents, Box<dyn Error>> {
    let events_rx = events.events();
    
    let source_configuration = SourceConfigurationRegistryOptions::builder()
        .client(client)
        .events(events_rx)
        .build()
        .into();
    
    let current_location = CurrentLocationConfigurator::new();
    
    let shared_filter_handlers = SharedFilterHandlersManagerOptions::builder()
        .routing(source_configuration.routing())
        .build()
        .into();
    
    let backends_configurator = BackendConfiguratorOptions::builder()
        .current_location(current_location.current_location())
        .backends(source_configuration.backends())
        .build()
        .into();
    
    Ok(GatewayComponents {
        source_configuration,
        shared_filter_handlers,
        backends_configurator,
        current_location,
    })
}

struct GatewayHandles {
    tasks: JoinSet<()>,
    monitors: Vec<ComponentMonitor>,
}

async fn start_gateway_components(components: GatewayComponents) -> Result<GatewayHandles, Box<dyn Error>> {
    let mut tasks = JoinSet::new();
    
    // Set initial location
    let current_location_tx = components.current_location.start();
    current_location_tx.send(Arc::new(
        TopologyLocation::builder()
            .zone("us-west-1".to_string())
            .node("a".to_string())
            .build(),
    ))?;
    
    // Start all components
    let shared_filter_handle = components.shared_filter_handlers.start();
    let backends_handle = components.backends_configurator.start();
    let source_config_handle = components.source_configuration.start();
    
    // Spawn component tasks
    tasks.spawn(shared_filter_handle.stopped());
    tasks.spawn(backends_handle.stopped());
    tasks.spawn(source_config_handle.stopped());
    
    // Create component monitors for status tracking
    let monitors = create_component_monitors(&components);
    
    Ok(GatewayHandles { tasks, monitors })
}

async fn wait_for_completion(mut handles: GatewayHandles) {
    while let Some(result) = handles.tasks.join_next().await {
        match result {
            Ok(_) => tracing::warn!("Gateway component completed unexpectedly"),
            Err(e) => tracing::error!("Gateway component failed: {}", e),
        }
        
        if handles.tasks.is_empty() {
            tracing::error!("All gateway components stopped");
            break;
        }
    }
}
```

### 5. Component Status Monitoring

Simple component monitoring without connection management:

```rust
pub struct ComponentMonitor {
    name: String,
    status_rx: watch::Receiver<ComponentStatus>,
}

#[derive(Debug, Clone)]
pub enum ComponentStatus {
    Starting,
    Running,
    Degraded(String),
    Stopped(String),
}

impl ComponentMonitor {
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                match *self.status_rx.borrow() {
                    ComponentStatus::Running => {
                        tracing::debug!("Component {} is running normally", self.name);
                    }
                    ComponentStatus::Degraded(ref reason) => {
                        tracing::warn!("Component {} is degraded: {}", self.name, reason);
                    }
                    ComponentStatus::Stopped(ref reason) => {
                        tracing::error!("Component {} has stopped: {}", self.name, reason);
                        break;
                    }
                    ComponentStatus::Starting => {
                        tracing::info!("Component {} is starting", self.name);
                    }
                }
            }
        })
    }
}
```

## Data Models

### Configuration Enhancements

```rust
#[derive(Debug, Clone)]
pub struct RobustClientConfig {
    // Existing fields...
    pub internal_monitoring: InternalMonitoringConfig,
    pub startup_logging: StartupLoggingConfig,
}

#[derive(Debug, Clone)]
pub struct InternalMonitoringConfig {
    pub enabled: bool,
    pub check_interval: Duration,
    pub health_check_timeout: Duration,
    pub critical_failure_threshold: u32,
    pub log_heartbeat: bool,
}

#[derive(Debug, Clone)]
pub struct StartupLoggingConfig {
    pub log_connection_attempts: bool,
    pub log_validation_results: bool,
    pub log_background_operations: bool,
    pub startup_summary: bool,
}

impl Default for RobustClientConfig {
    fn default() -> Self {
        Self {
            // Robust defaults suitable for all environments
            internal_monitoring: InternalMonitoringConfig {
                enabled: true,
                check_interval: Duration::from_secs(30),
                health_check_timeout: Duration::from_secs(5),
                critical_failure_threshold: 10,
                log_heartbeat: false, // Reduce noise
            },
            startup_logging: StartupLoggingConfig {
                log_connection_attempts: true,
                log_validation_results: true,
                log_background_operations: true,
                startup_summary: true,
            },
        }
    }
}
```

## Error Handling

### Simplified Error Propagation

The gateway only handles critical errors that prevent basic operation:

```rust
#[derive(Debug, Error)]
pub enum GatewayStartupError {
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Component initialization failed: {0}")]
    ComponentInitialization(String),
    
    #[error("Critical resource unavailable: {0}")]
    CriticalResource(String),
}

// Client errors are handled internally and don't propagate to gateway
impl From<ConfigurationClientError> for GatewayStartupError {
    fn from(err: ConfigurationClientError) -> Self {
        match err {
            ConfigurationClientError::InvalidConfiguration(msg) => {
                GatewayStartupError::InvalidConfiguration(msg)
            }
            _ => {
                // All other client errors are handled internally
                // Gateway doesn't need to know about transport issues
                tracing::debug!("Client error handled internally: {}", err);
                GatewayStartupError::ComponentInitialization(
                    "Configuration client initialization completed with internal error handling".to_string()
                )
            }
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. **Client Factory Tests**: Verify factory methods create properly configured clients
2. **Internal Monitoring Tests**: Test monitoring loop behavior and logging
3. **Startup Manager Tests**: Test different startup modes and error handling
4. **Gateway Main Tests**: Test simplified main function logic

### Integration Tests

1. **End-to-End Gateway Tests**: Test complete gateway startup with mock services
2. **Client Self-Management Tests**: Verify clients handle all transport concerns internally
3. **Startup Resilience Tests**: Test gateway startup with various service availability scenarios

### Performance Tests

1. **Startup Time Tests**: Verify gateway starts quickly regardless of service availability
2. **Memory Usage Tests**: Ensure internal monitoring doesn't consume excessive resources
3. **Log Volume Tests**: Verify logging is comprehensive but not excessive

## Migration Strategy

### Phase 1: Enhance Client Self-Management
- Add internal monitoring to existing robust clients
- Implement enhanced startup handling
- Add comprehensive internal logging

### Phase 2: Simplify Gateway Main
- Remove connection monitoring from gateway
- Remove health checking logic
- Remove custom timeout handling
- Remove connection status logging

### Phase 3: Clean Up and Optimize
- Remove unused monitoring infrastructure
- Optimize client startup performance
- Add comprehensive testing

## Backward Compatibility

The changes maintain full backward compatibility:
- Existing client APIs remain unchanged
- Configuration options are additive
- Gateway behavior improves without breaking changes
- All robustness features continue to work as before

The gateway becomes simpler and more reliable while the clients become more capable and self-sufficient.