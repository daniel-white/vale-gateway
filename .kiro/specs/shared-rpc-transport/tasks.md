# Implementation Plan

- [x] 1. Add dependencies and prepare infrastructure
  - Add getset and typed_builder dependencies to rpc-client Cargo.toml
  - Update existing imports to include core utilities (handles, broadcast channels)
  - Create basic RpcTransportError and RpcTransportConfig types
  - _Requirements: 4.1, 4.2_

- [x] 2. Implement RpcTransport core structure
  - [x] 2.1 Create RpcTransport struct with getset and typed_builder
    - Define RpcTransport with LayeredClient, config, handles, and event sender fields
    - Implement new() method that creates LayeredClient using existing EnhancedWsClientBuilder
    - Add core broadcast channel for event distribution
    - _Requirements: 1.1, 1.3, 4.2, 4.3_
  
  - [x] 2.2 Implement RpcTransportConfig with getset and typed_builder
    - Create config struct with robust_config, address, and event_buffer_size fields
    - Add From<ConfigurationTransportOptions> implementation for compatibility
    - Add validation methods reusing existing config validation
    - _Requirements: 1.4, 6.3_
  
  - [x] 2.3 Add connection lifecycle management using core handles
    - Implement connection monitoring using existing MonitoringManager
    - Use core handles for task management and graceful shutdown
    - Add connection state tracking using core sync primitives
    - _Requirements: 1.4, 4.1, 4.5, 5.2_

- [x] 3. Update ConfigurationClient to use RpcTransport
  - [x] 3.1 Modify ConfigurationClient struct with getset and typed_builder
    - Replace transport field with RpcTransport
    - Add listener_ref field with getset accessor
    - Remove connect() methods, add new() constructor
    - _Requirements: 6.1, 6.4_
  
  - [x] 3.2 Update client methods to use RpcTransport
    - Modify listener(), route(), backend(), shared_filter() to use transport.client()
    - Reuse existing request building and tracing logic
    - Ensure error handling remains the same
    - _Requirements: 1.3, 6.2_
  
  - [x] 3.3 Add monitoring integration for ConfigurationClient
    - Integrate with RpcTransport monitoring using existing monitoring_status() method
    - Reuse existing stop_monitoring() and is_monitoring() methods
    - Ensure monitoring works with shared connection
    - _Requirements: 5.1, 5.5_

- [x] 4. Update ConfigurationEventsClient to use RpcTransport
  - [x] 4.1 Modify ConfigurationEventsClient struct with getset and typed_builder
    - Replace transport field with RpcTransport
    - Add listener_ref and event_receiver fields with getset accessors
    - Remove connect() methods, add new() constructor
    - _Requirements: 6.1, 6.4_
  
  - [x] 4.2 Implement event subscription using RpcTransport
    - Modify start() method to use transport.client() for subscription
    - Use core broadcast channels for event distribution from RpcTransport
    - Reuse existing event processing logic with core handles
    - _Requirements: 1.3, 2.2, 4.3_
  
  - [x] 4.3 Update events() method to use shared event distribution
    - Modify events() method to return receiver from RpcTransport event_sender
    - Ensure ConfigurationEventsReceiver works with shared events
    - Maintain existing event receiver API and behavior
    - _Requirements: 2.3, 4.3, 6.2_
  
  - [x] 4.4 Add resilient connection handling for events
    - Implement graceful reconnection using existing reconnection layers
    - Add event queuing during connection failures using core utilities
    - Ensure events client doesn't fail on initial connection failure
    - _Requirements: 2.1, 2.2, 2.4_

- [ ] 5. Reorganize transport module structure
  - [ ] 5.1 Create new transport module organization
    - Create transport/rpc.rs for RpcTransport implementation
    - Move robustness layers to transport/layers/robustness/ submodule
    - Move connection management to transport/layers/connection/ submodule
    - Move monitoring to transport/layers/monitoring/ submodule
    - _Requirements: 3.1, 3.2_
  
  - [ ] 5.2 Update transport/mod.rs exports
    - Export RpcTransport and related types
    - Update layer exports to use new submodule structure
    - Remove unused ClientWrapper and ConfigurationTransport exports
    - _Requirements: 3.3, 3.4_
  
  - [ ] 5.3 Clean up unused transport code
    - Remove ConfigurationTransport and ConfigurationTransportOptions
    - Remove ClientWrapper enum and related wrapper logic
    - Remove duplicate connection management code
    - _Requirements: 3.3, 3.5_

- [ ] 6. Update client builders and factory methods
  - [ ] 6.1 Remove connect() methods from both clients
    - Remove ConfigurationClient::connect() and related methods
    - Remove ConfigurationEventsClient::connect() and related methods
    - Remove ConfigurationClientBuilder and related builder code
    - _Requirements: 6.1, 6.5_
  
  - [ ] 6.2 Add RpcTransport factory methods
    - Add RpcTransport::new() with robust configuration
    - Add convenience methods like production(), development()
    - Reuse existing RobustClientConfig presets
    - _Requirements: 1.5, 6.4_
  
  - [ ] 6.3 Update error handling for new constructor pattern
    - Ensure RpcTransportError integrates with existing error classification
    - Update client error handling to work with transport parameter approach
    - Maintain existing error types and behavior for client methods
    - _Requirements: 6.2, 6.3_

- [ ] 7. Add comprehensive testing
  - [ ] 7.1 Create RpcTransport unit tests
    - Test RpcTransport creation with various configurations
    - Test connection lifecycle management and monitoring
    - Test event distribution using core broadcast channels
    - _Requirements: 1.1, 1.3, 4.2_
  
  - [ ] 7.2 Create client integration tests
    - Test ConfigurationClient with RpcTransport parameter
    - Test ConfigurationEventsClient with RpcTransport parameter
    - Test both clients sharing the same RpcTransport instance
    - _Requirements: 1.2, 6.1, 6.2_
  
  - [ ] 7.3 Add resilience and error handling tests
    - Test events client graceful connection handling
    - Test connection recovery and reconnection scenarios
    - Test monitoring integration with shared connection
    - _Requirements: 2.1, 2.2, 5.1_

- [ ] 8. Performance optimization and monitoring
  - [ ] 8.1 Optimize RpcTransport performance
    - Profile connection setup and event distribution overhead
    - Optimize core broadcast channel usage for high-throughput events
    - Ensure minimal performance impact from getset and typed_builder
    - _Requirements: 5.3, 5.4_
  
  - [ ] 8.2 Add comprehensive monitoring for RpcTransport
    - Integrate existing monitoring with shared connection metrics
    - Add usage statistics for both configuration and events traffic
    - Implement connection health reporting for debugging
    - _Requirements: 5.1, 5.2, 5.5_
  
  - [ ] 8.3 Validate resource usage improvements
    - Measure connection count reduction (should be 50% fewer connections)
    - Measure memory usage improvement from eliminating duplicate transports
    - Verify startup time improvements from single connection establishment
    - _Requirements: 1.1, 1.2_

- [x] 9. Update gateway to use new RpcTransport pattern
  - [x] 9.1 Update gateway main.rs to use RpcTransport constructor pattern
    - Replace ConfigurationClient::connect() with RpcTransport::new() + ConfigurationClient::new()
    - Replace ConfigurationEventsClient::connect() with ConfigurationEventsClient::new()
    - Share single RpcTransport instance between both clients
    - _Requirements: 1.1, 1.2, 6.1_
  
  - [x] 9.2 Update gateway error handling for new pattern
    - Handle RpcTransportError in gateway startup
    - Ensure graceful handling of transport creation failures
    - Maintain existing gateway resilience behavior
    - _Requirements: 2.1, 6.3_