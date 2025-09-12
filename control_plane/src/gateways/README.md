# Gateway Configuration Merging System

This module provides a comprehensive system for merging GatewayClassParameters and GatewayParameters into final Kubernetes resources for Vale Gateway.

## Architecture

The system consists of several key components:

### Core Components

1. **ConfigurationMerger** (`core/src/config/gateway/merge.rs`)
   - Handles the merging logic with proper precedence
   - Gateway Parameters > GatewayClass Parameters > Defaults

2. **GatewayResourceConfiguration** (`core/src/config/gateway/resources.rs`)
   - Represents the final configuration for a gateway instance
   - Includes all Kubernetes resources (Deployment, Service, ConfigMap)

3. **GatewayConfigurationMergerService** (`configuration_merger.rs`)
   - Reactive service that watches for configuration changes
   - Produces merged configurations automatically

4. **GatewayResourceSyncService** (`resource_sync.rs`)
   - Syncs the final resources to Kubernetes cluster
   - Uses server-side apply for efficient updates

5. **GatewayOrchestrator** (`orchestrator.rs`)
   - Main entry point that coordinates all services

## Configuration Precedence

The system follows a clear precedence hierarchy:

1. **Gateway Parameters** (highest priority) - Instance-specific overrides
2. **GatewayClass Parameters** (medium priority) - Cluster-wide defaults
3. **System Defaults** (lowest priority) - Built-in defaults

### Example

```yaml
# GatewayClassParameters (cluster defaults)
spec:
  common:
    deployment:
      replicas: 2
      image:
        repository: "my-registry/vale-gateway"
        tag: "v1.0.0"
    gateway:
      logLevel: Info

---
# GatewayParameters (instance overrides)
spec:
  common:
    deployment:
      replicas: 3              # Overrides class
      image:
        tag: "v1.1.0"          # Overrides tag, keeps repo from class
    gateway:
      logLevel: Debug          # Overrides class
```

**Final Result:**
- Replicas: 3 (from gateway)
- Image: "my-registry/vale-gateway:v1.1.0" (repo from class, tag from gateway)
- Log Level: Debug (from gateway)

## Usage

### Integration

The system is automatically started in the control plane:

```rust
// In main.rs
crate::gateways::GatewayOrchestrator::spawn_all(&task_builder, options, &kube_client_rx);
```

### Manual Configuration Merging

```rust
use vg_core::config::gateway::ConfigurationMerger;

let merged = ConfigurationMerger::merge(
    "my-gateway",
    "production",
    Some(&gateway_class_params),
    Some(&gateway_params),
    None, // base metadata
);

// Access merged configuration
println!("Image: {}:{}", merged.image_repository(), merged.image_tag());
```

### Building Resource Configurations

```rust
use vg_core::config::gateway::GatewayResourceConfiguration;

let config = GatewayResourceConfiguration::builder()
    .name("my-gateway")
    .namespace("production")
    .deployment(deployment)
    .service(service)
    .config_map(config_map)
    .image_repository("vale-gateway")
    .image_tag("latest")
    .gateway_config(gateway_config)
    .build();

// Apply common labels
config.apply_common_labels();
```

## Supported Configuration Fields

### Deployment Configuration
- Replicas
- Image repository and tag
- Image pull policy
- Deployment strategy
- Environment variables (log level)

### Service Configuration
- Service spec (ports, type, etc.)

### Gateway Configuration
- Log level
- Instrumentation (OpenTelemetry)
- Listeners

### Metadata
- Labels and annotations
- Gateway class association
- Management flags

## Extensibility

To add new configuration fields:

1. Add the field to the appropriate API types in `vg_api::v1alpha1`
2. Update the merge functions in `ConfigurationMerger`
3. Add any necessary resource generation logic

## Testing

The system includes comprehensive test coverage across all components:

### Unit Tests

```bash
# Test core configuration merging logic
cargo test --package vg-core config::gateway

# Test control plane gateway controllers
cargo test --package vg-control-plane gateways

# Test specific sync controllers
cargo test --package vg-control-plane gateways::sync
```

### Test Coverage

#### Core Configuration Tests (`core/src/config/gateway/tests.rs`)
- **Configuration Merging**: Tests precedence rules (Gateway > Class > Defaults)
- **Resource Configuration**: Tests TypedBuilder patterns and getters
- **Edge Cases**: Tests with empty parameters, missing configurations
- **Integration**: End-to-end configuration flow testing

#### Sync Controller Tests (`control_plane/src/gateways/sync/`)
- **Deployment Sync**: Tests Kubernetes Deployment preparation and validation
- **Service Sync**: Tests Service configuration with default ports and selectors  
- **ConfigMap Sync**: Tests configuration validation and YAML generation
- **Controller Logic**: Tests sync statistics, error handling, and dry-run mode

#### Integration Tests (`control_plane/src/gateways/integration_tests.rs`)
- **Complete Workflow**: Tests full gateway lifecycle from parameters to K8s resources
- **Multiple Gateways**: Tests handling of multiple gateway instances
- **Resource Naming**: Tests consistent naming across all resource types
- **Statistics**: Tests sync operation monitoring and aggregation

### Test Structure

```rust
// Example test showing configuration precedence
#[test]
fn test_configuration_precedence() {
    let gateway_class_params = create_gateway_class_params();
    let gateway_params = create_gateway_params();
    
    let merged = ConfigurationMerger::merge(
        "test-gateway",
        "production", 
        Some(&gateway_class_params),
        Some(&gateway_params),
        None,
    );
    
    // Gateway parameters should override class parameters
    assert_eq!(merged.deployment().spec.unwrap().replicas, Some(3));
    assert_eq!(merged.image_tag(), "v1.1.0"); // From gateway
    assert_eq!(merged.image_repository(), "class-repo"); // From class
}
```

### Mock Testing

For Kubernetes API interactions, the tests use:
- **Dry Run Mode**: Tests sync logic without actual API calls
- **Resource Preparation**: Tests resource transformation and validation
- **Error Scenarios**: Tests error handling and recovery

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test module
cargo test gateways::sync::deployment_sync

# Run integration tests only
cargo test integration_tests
```

## Monitoring

The system emits tracing logs for:
- Configuration merging operations
- Resource synchronization
- Error conditions

Use `RUST_LOG=debug` to see detailed operation logs.