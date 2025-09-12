use crate::gateways::GatewayOrchestrator;
use crate::gateways::configuration_merger::ConfigurationMerger;
use crate::options::Options;
use std::sync::Arc;
use vg_core::sync::signal::signal;
use vg_core::task::Builder as TaskBuilder;

pub fn example_integration() {
    // This is an example of how you would integrate the gateway orchestrator
    // into your main control plane application

    let task_builder = TaskBuilder::default();
    let options = Arc::new(Options::default());

    // Create a mock kube client receiver (in real usage, this would come from your kube client setup)
    let (kube_client_tx, kube_client_rx) = signal("kube_client");

    // Create a mock IPC configuration
    let ipc_config = Arc::new(
        vg_core::ipc::IpcConfiguration::builder()
            .addr(std::net::SocketAddr::from(([127, 0, 0, 1], 8080)))
            .build(),
    );

    // Create mock IPC services
    let (event_sender, _events_factory) = crate::ipc::events::events_channel();
    let (_reader, gateway_manager) = crate::ipc::gateways::create_gateway_configuration_services();
    let mock_ipc_services = Arc::new(
        crate::ipc::IpcServices::builder()
            .events(event_sender)
            .gateway_configuration_manager(gateway_manager)
            .port(vg_core::net::Port::new(
                std::num::NonZeroU16::new(8080).unwrap(),
            ))
            .build(),
    );

    // Start the gateway orchestrator
    GatewayOrchestrator::spawn_all(
        &task_builder,
        options,
        &kube_client_rx,
        ipc_config,
        mock_ipc_services,
    );

    // The orchestrator will now:
    // 1. Watch for GatewayClass and Gateway resources
    // 2. Collect GatewayClassParameters and GatewayParameters
    // 3. Merge configurations with proper precedence
    // 4. Generate Kubernetes resources (Deployments, Services, ConfigMaps)
    // 5. Sync these resources to the cluster
}

/// Example showing the configuration precedence
pub fn example_configuration_precedence() {
    use vg_api::v1alpha1::{
        CommonGatewayParameterSpec, GatewayClassParameters, GatewayClassParametersSpec,
        GatewayConfiguration, GatewayDeployment, GatewayParameters, GatewayParametersSpec, Image,
        LogLevel,
    };
    // Example GatewayClassParameters (cluster-wide defaults)
    let mut gateway_class_params = GatewayClassParameters::default();
    gateway_class_params.spec = GatewayClassParametersSpec {
        common: CommonGatewayParameterSpec {
            deployment: Some(GatewayDeployment {
                replicas: Some(2),
                image: Some(Image {
                    repository: Some("my-registry/vale-gateway".to_string()),
                    tag: Some("v1.0.0".to_string()),
                }),
                image_pull_policy: None,
                strategy: None,
            }),
            gateway: Some(GatewayConfiguration {
                log_level: Some(LogLevel::Info),
                ..Default::default()
            }),
        },
        cluster_name: Some("production".to_string()),
    };

    // Example GatewayParameters (instance-specific overrides)
    let mut gateway_params = GatewayParameters::default();
    gateway_params.spec = GatewayParametersSpec {
        common: Some(CommonGatewayParameterSpec {
            deployment: Some(GatewayDeployment {
                replicas: Some(3), // Override: 3 replicas instead of 2
                image: Some(Image {
                    repository: None,                // Inherit from class
                    tag: Some("v1.1.0".to_string()), // Override: newer version
                }),
                image_pull_policy: None,
                strategy: None,
            }),
            gateway: Some(GatewayConfiguration {
                log_level: Some(LogLevel::Debug), // Override: debug instead of info
                ..Default::default()
            }),
        }),
        service: None,
    };

    // Merge the configurations
    let merged = ConfigurationMerger::merge(
        "example-gateway",
        "production",
        Some(&gateway_class_params),
        Some(&gateway_params),
        None,
    );

    // The result will have:
    // - 3 replicas (from gateway params)
    // - Image: my-registry/vale-gateway:v1.1.0 (repo from class, tag from gateway)
    // - Log level: Debug (from gateway params)

    println!("Merged configuration:");
    println!(
        "- Replicas: {:?}",
        merged.deployment().spec.as_ref().unwrap().replicas
    );
    println!(
        "- Image: {}:{}",
        merged.image_repository(),
        merged.image_tag()
    );
    println!("- Log level: {:?}", merged.gateway_config().log_level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_configuration_precedence() {
        // This test ensures the example compiles and runs
        example_configuration_precedence();
    }
}
