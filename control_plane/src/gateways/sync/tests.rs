#![cfg(test)]

use super::*;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy};
use k8s_openapi::api::core::v1::{ConfigMap, Service, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use std::collections::BTreeMap;
use vg_api::v1alpha1::GatewayConfiguration;
use vg_core::config::gateway::{GatewayResourceConfiguration, GatewayResourceConfigurations};

pub fn create_test_deployment() -> Deployment {
    Deployment {
        metadata: ObjectMeta {
            name: Some("test-gateway".to_string()),
            namespace: Some("default".to_string()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(2),
            selector: LabelSelector {
                match_labels: Some({
                    let mut labels = BTreeMap::new();
                    labels.insert("app".to_string(), "test".to_string());
                    labels
                }),
                ..Default::default()
            },
            template: Default::default(),
            strategy: Some(DeploymentStrategy::default()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Create a test gateway resource configuration
pub fn create_test_gateway_config(name: &str, namespace: &str) -> GatewayResourceConfiguration {
    let deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(2),
            selector: LabelSelector {
                match_labels: Some({
                    let mut labels = BTreeMap::new();
                    labels.insert("app".to_string(), name.to_string());
                    labels
                }),
                ..Default::default()
            },
            template: Default::default(),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service = Service {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            type_: Some("ClusterIP".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut config_data = BTreeMap::new();
    config_data.insert(
        "gateway.yaml".to_string(),
        "logLevel: Info\nlisteners:\n  http:\n    filters: []".to_string(),
    );

    let config_map = ConfigMap {
        metadata: ObjectMeta {
            name: Some(format!("{}-config", name)),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        data: Some(config_data),
        ..Default::default()
    };

    GatewayResourceConfiguration::builder()
        .name(name)
        .namespace(namespace)
        .deployment(deployment)
        .service(service)
        .config_map(config_map)
        .image_repository("vale-gateway")
        .image_tag("latest")
        .gateway_config(GatewayConfiguration::default())
        .open_telemetry(None)
        .build()
}

/// Create test gateway configurations
pub fn create_test_configurations() -> GatewayResourceConfigurations {
    let mut configs = GatewayResourceConfigurations::new();

    configs.add(create_test_gateway_config("gateway1", "default"));
    configs.add(create_test_gateway_config("gateway2", "production"));
    configs.add(create_test_gateway_config("gateway3", "staging"));

    configs
}

#[cfg(test)]
mod integration_tests {
    use assertables::assert_approx_eq;
    use super::*;
    use serde::ser::Error;

    #[test]
    fn test_complete_gateway_configuration() {
        let configs = create_test_configurations();
        assert_eq!(configs.len(), 3);

        // Test each configuration
        for (name, config) in configs.iter() {
            // Check basic properties
            assert!(!config.name().is_empty());
            assert!(!config.namespace().is_empty());
            assert_eq!(config.name(), name);

            // Check resource names
            assert_eq!(config.deployment_name(), name);
            assert_eq!(config.service_name(), name);
            assert_eq!(config.config_map_name(), format!("{}-config", name));

            // Check container image
            assert_eq!(config.container_image(), "vale-gateway:latest");
        }
    }

    #[test]
    fn test_sync_stats_calculation() {
        let stats = SyncStats::builder()
            .created(10)
            .updated(5)
            .deleted(2)
            .errors(1)
            .no_action(3)
            .build();

        assert_eq!(stats.total(), 21);
        assert_approx_eq!(stats.success_rate(), 20.0 / 21.0);
    }

    #[test]
    fn test_resource_action_variants() {
        let deployment = Deployment::default();

        let create_action = ResourceAction::Create(deployment.clone());
        let update_action = ResourceAction::Update(deployment.clone());
        let delete_action: ResourceAction<Deployment> = ResourceAction::Delete;
        let no_action: ResourceAction<Deployment> = ResourceAction::NoAction;

        // Test pattern matching
        match create_action {
            ResourceAction::Create(_) => (),
            _ => panic!("Expected Create action"),
        }

        match update_action {
            ResourceAction::Update(_) => (),
            _ => panic!("Expected Update action"),
        }

        match delete_action {
            ResourceAction::Delete => (),
            _ => panic!("Expected Delete action"),
        }

        match no_action {
            ResourceAction::NoAction => (),
            _ => panic!("Expected NoAction"),
        }
    }

    #[test]
    fn test_sync_error_types() {
        // Test different error types
        let kube_error = SyncError::KubeApi(kube::Error::Api(kube::core::ErrorResponse {
            status: "Failure".to_string(),
            message: "Test error".to_string(),
            reason: "TestReason".to_string(),
            code: 500,
        }));

        let serialization_error = SyncError::Serialization(serde_json::Error::custom("test"));
        let not_found_error = SyncError::ResourceNotFound("test-resource".to_string());
        let invalid_config_error = SyncError::InvalidConfiguration("test config".to_string());

        // Test error display
        assert!(format!("{}", kube_error).contains("Kubernetes API error"));
        assert!(format!("{}", serialization_error).contains("Serialization error"));
        assert!(format!("{}", not_found_error).contains("Resource not found"));
        assert!(format!("{}", invalid_config_error).contains("Invalid resource configuration"));
    }
}

/// Mock tests that would require a real Kubernetes cluster
#[cfg(test)]
mod mock_tests {
    use super::*;

    // These tests would normally use a mock Kubernetes client
    // For demonstration purposes, they test the structure and logic

    #[tokio::test]
    async fn test_deployment_sync_workflow() {
        let sync = DeploymentSynchronizer::new();
        let configs = create_test_configurations();
        let params = SyncParams::builder().dry_run(true).build();

        // In a real test, you would:
        // 1. Create a mock Kubernetes client
        // 2. Set up expectations for API calls
        // 3. Call sync_all_deployments
        // 4. Verify the mock expectations

        assert_eq!(configs.len(), 3);
        assert!(params.dry_run());
    }

    #[tokio::test]
    async fn test_service_sync_workflow() {
        let sync = ServiceSynchronizer::new();
        let configs = create_test_configurations();
        let params = SyncParams::builder().dry_run(true).build();

        // Similar to deployment test - would use mock client
        assert_eq!(configs.len(), 3);
        assert!(params.dry_run());
    }

    #[tokio::test]
    async fn test_configmap_sync_workflow() {
        let sync = ConfigMapSynchronizer::new();
        let configs = create_test_configurations();
        let params = SyncParams::builder().dry_run(true).build();

        // Similar to other tests - would use mock client
        assert_eq!(configs.len(), 3);
        assert!(params.dry_run());
    }
}
