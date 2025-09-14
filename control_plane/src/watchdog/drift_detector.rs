use std::collections::BTreeMap;

use k8s_openapi::{
    api::{
        apps::v1::Deployment,
        core::v1::{ConfigMap, Service},
    },
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::Resource;
use serde_json::Value;
use tracing::{debug, warn};

use crate::watchdog::{data_models::*, error::WatchdogError};

/// Trait for detecting configuration drift in Kubernetes resources
pub trait DriftDetector<T> {
    /// Detect drift between the expected and actual resource states
    fn detect_drift(
        &self,
        expected: &T,
        actual: Option<&T>,
        resource_name: &str,
        namespace: &str,
    ) -> Result<Option<ResourceDrift<T>>, WatchdogError>
    where
        T: Clone;

    /// Validate that a resource is owned by the gateway
    fn validate_ownership(&self, resource: &T) -> Result<bool, WatchdogError>;

    /// Get the resource type name for logging
    fn resource_type_name(&self) -> &'static str;
}

/// Default implementation of drift detection for Kubernetes resources
pub struct DefaultDriftDetector {
    /// Labels that indicate gateway ownership
    gateway_labels: BTreeMap<String, String>,
    /// Annotations that indicate gateway ownership
    gateway_annotations: BTreeMap<String, String>,
}

impl DefaultDriftDetector {
    /// Create a new drift detector with gateway ownership markers
    pub fn new(
        gateway_labels: BTreeMap<String, String>,
        gateway_annotations: BTreeMap<String, String>,
    ) -> Self {
        Self {
            gateway_labels,
            gateway_annotations,
        }
    }

    /// Create a drift detector with default gateway labels
    pub fn with_defaults() -> Self {
        let mut gateway_labels = BTreeMap::new();
        gateway_labels.insert(
            "app.kubernetes.io/managed-by".to_string(),
            "vale-gateway".to_string(),
        );
        gateway_labels.insert("vale-gateway.io/managed".to_string(), "true".to_string());

        let mut gateway_annotations = BTreeMap::new();
        gateway_annotations.insert("vale-gateway.io/resource-hash".to_string(), "".to_string());

        Self::new(gateway_labels, gateway_annotations)
    }

    /// Check if the resource has the required gateway ownership labels and annotations
    fn has_gateway_ownership_markers(&self, metadata: &ObjectMeta) -> bool {
        // Check required labels
        if let Some(labels) = &metadata.labels {
            for (key, expected_value) in &self.gateway_labels {
                if key == "vale-gateway.io/resource-hash" {
                    continue; // Skip hash comparison for ownership check
                }
                match labels.get(key) {
                    Some(actual_value) if actual_value == expected_value => continue,
                    _ => {
                        debug!(
                            "Missing or incorrect gateway label: {} = {}",
                            key, expected_value
                        );
                        return false;
                    }
                }
            }
        } else {
            debug!("Resource has no labels");
            return false;
        }

        // Check required annotations
        if let Some(annotations) = &metadata.annotations {
            for (key, _) in &self.gateway_annotations {
                if !annotations.contains_key(key) {
                    debug!("Missing gateway annotation: {}", key);
                    return false;
                }
            }
        } else {
            debug!("Resource has no annotations");
            return false;
        }

        true
    }

    /// Compare two resources for semantic differences (ignoring metadata changes)
    fn resources_semantically_equal<T>(&self, expected: &T, actual: &T) -> bool
    where
        T: serde::Serialize,
    {
        // Serialize both resources to JSON for comparison
        let expected_json = match serde_json::to_value(expected) {
            Ok(json) => json,
            Err(e) => {
                warn!("Failed to serialize expected resource: {}", e);
                return false;
            }
        };

        let actual_json = match serde_json::to_value(actual) {
            Ok(json) => json,
            Err(e) => {
                warn!("Failed to serialize actual resource: {}", e);
                return false;
            }
        };

        // Remove metadata fields that should be ignored in drift detection
        let expected_normalized = self.normalize_for_comparison(expected_json);
        let actual_normalized = self.normalize_for_comparison(actual_json);

        expected_normalized == actual_normalized
    }

    /// Normalize a resource JSON for comparison by removing fields that should be ignored
    fn normalize_for_comparison(&self, mut resource: Value) -> Value {
        if let Some(obj) = resource.as_object_mut() {
            // Remove metadata fields that change during normal operation
            if let Some(metadata) = obj.get_mut("metadata").and_then(|m| m.as_object_mut()) {
                metadata.remove("resourceVersion");
                metadata.remove("uid");
                metadata.remove("generation");
                metadata.remove("creationTimestamp");
                metadata.remove("managedFields");
                metadata.remove("selfLink");

                // Remove dynamic annotations that shouldn't trigger drift detection
                if let Some(annotations) = metadata
                    .get_mut("annotations")
                    .and_then(|a| a.as_object_mut())
                {
                    annotations.remove("kubectl.kubernetes.io/last-applied-configuration");
                    annotations.remove("deployment.kubernetes.io/revision");
                }
            }

            // Remove status field as it's managed by Kubernetes
            obj.remove("status");
        }

        resource
    }
}

impl DriftDetector<ConfigMap> for DefaultDriftDetector {
    fn detect_drift(
        &self,
        expected: &ConfigMap,
        actual: Option<&ConfigMap>,
        resource_name: &str,
        namespace: &str,
    ) -> Result<Option<ResourceDrift<ConfigMap>>, WatchdogError> {
        match actual {
            Some(actual_resource) => {
                // Check if resource is owned by gateway
                if !self.validate_ownership(actual_resource)? {
                    return Ok(None); // Not our resource, ignore
                }

                // Check for modifications
                if !self.resources_semantically_equal(expected, actual_resource) {
                    debug!(
                        "ConfigMap {}/{} has been modified",
                        namespace, resource_name
                    );

                    Ok(Some(
                        ResourceDrift::builder()
                            .resource_name(resource_name)
                            .namespace(namespace)
                            .actual_resource(Some(actual_resource.clone()))
                            .expected_resource(expected.clone())
                            .drift_type(DriftType::Modified)
                            .resource_type("ConfigMap")
                            .build(),
                    ))
                } else {
                    Ok(None) // No drift detected
                }
            }
            None => {
                // Resource was deleted
                debug!("ConfigMap {}/{} was deleted", namespace, resource_name);

                Ok(Some(
                    ResourceDrift::builder()
                        .resource_name(resource_name)
                        .namespace(namespace)
                        .actual_resource(None)
                        .expected_resource(expected.clone())
                        .drift_type(DriftType::Deleted)
                        .resource_type("ConfigMap")
                        .build(),
                ))
            }
        }
    }

    fn validate_ownership(&self, resource: &ConfigMap) -> Result<bool, WatchdogError> {
        let metadata = &resource.metadata;
        Ok(self.has_gateway_ownership_markers(metadata))
    }

    fn resource_type_name(&self) -> &'static str {
        "ConfigMap"
    }
}

impl DriftDetector<Service> for DefaultDriftDetector {
    fn detect_drift(
        &self,
        expected: &Service,
        actual: Option<&Service>,
        resource_name: &str,
        namespace: &str,
    ) -> Result<Option<ResourceDrift<Service>>, WatchdogError> {
        match actual {
            Some(actual_resource) => {
                // Check if resource is owned by gateway
                if !self.validate_ownership(actual_resource)? {
                    return Ok(None); // Not our resource, ignore
                }

                // Check for modifications
                if !self.resources_semantically_equal(expected, actual_resource) {
                    debug!("Service {}/{} has been modified", namespace, resource_name);

                    Ok(Some(
                        ResourceDrift::builder()
                            .resource_name(resource_name)
                            .namespace(namespace)
                            .actual_resource(Some(actual_resource.clone()))
                            .expected_resource(expected.clone())
                            .drift_type(DriftType::Modified)
                            .resource_type("Service")
                            .build(),
                    ))
                } else {
                    Ok(None) // No drift detected
                }
            }
            None => {
                // Resource was deleted
                debug!("Service {}/{} was deleted", namespace, resource_name);

                Ok(Some(
                    ResourceDrift::builder()
                        .resource_name(resource_name)
                        .namespace(namespace)
                        .actual_resource(None)
                        .expected_resource(expected.clone())
                        .drift_type(DriftType::Deleted)
                        .resource_type("Service")
                        .build(),
                ))
            }
        }
    }

    fn validate_ownership(&self, resource: &Service) -> Result<bool, WatchdogError> {
        let metadata = &resource.metadata;
        Ok(self.has_gateway_ownership_markers(metadata))
    }

    fn resource_type_name(&self) -> &'static str {
        "Service"
    }
}

impl DriftDetector<Deployment> for DefaultDriftDetector {
    fn detect_drift(
        &self,
        expected: &Deployment,
        actual: Option<&Deployment>,
        resource_name: &str,
        namespace: &str,
    ) -> Result<Option<ResourceDrift<Deployment>>, WatchdogError> {
        match actual {
            Some(actual_resource) => {
                // Check if resource is owned by gateway
                if !self.validate_ownership(actual_resource)? {
                    return Ok(None); // Not our resource, ignore
                }

                // Check for modifications
                if !self.resources_semantically_equal(expected, actual_resource) {
                    debug!(
                        "Deployment {}/{} has been modified",
                        namespace, resource_name
                    );

                    Ok(Some(
                        ResourceDrift::builder()
                            .resource_name(resource_name)
                            .namespace(namespace)
                            .actual_resource(Some(actual_resource.clone()))
                            .expected_resource(expected.clone())
                            .drift_type(DriftType::Modified)
                            .resource_type("Deployment")
                            .build(),
                    ))
                } else {
                    Ok(None) // No drift detected
                }
            }
            None => {
                // Resource was deleted
                debug!("Deployment {}/{} was deleted", namespace, resource_name);

                Ok(Some(
                    ResourceDrift::builder()
                        .resource_name(resource_name)
                        .namespace(namespace)
                        .actual_resource(None)
                        .expected_resource(expected.clone())
                        .drift_type(DriftType::Deleted)
                        .resource_type("Deployment")
                        .build(),
                ))
            }
        }
    }

    fn validate_ownership(&self, resource: &Deployment) -> Result<bool, WatchdogError> {
        let metadata = &resource.metadata;
        Ok(self.has_gateway_ownership_markers(metadata))
    }

    fn resource_type_name(&self) -> &'static str {
        "Deployment"
    }
}

/// Helper function to detect unexpected resource creation
pub fn detect_unexpected_creation<T>(
    detector: &impl DriftDetector<T>,
    actual_resource: &T,
    expected_resources: &[T],
    resource_name: &str,
    namespace: &str,
) -> Result<Option<ResourceDrift<T>>, WatchdogError>
where
    T: Clone + Resource,
    T::DynamicType: Default,
{
    // Check if this resource is owned by the gateway
    if !detector.validate_ownership(actual_resource)? {
        return Ok(None); // Not our resource, ignore
    }

    // Check if this resource is expected
    let resource_exists_in_expected = expected_resources.iter().any(|expected| {
        expected.meta().name.as_ref() == Some(&resource_name.to_string())
            && expected.meta().namespace.as_ref() == Some(&namespace.to_string())
    });

    if !resource_exists_in_expected {
        debug!(
            "Unexpected {} {}/{} was created",
            detector.resource_type_name(),
            namespace,
            resource_name
        );

        // For unexpected creation, we use the actual resource as both actual and expected
        // since we need to decide whether to delete it or keep it
        Ok(Some(
            ResourceDrift::builder()
                .resource_name(resource_name)
                .namespace(namespace)
                .actual_resource(Some(actual_resource.clone()))
                .expected_resource(actual_resource.clone())
                .drift_type(DriftType::UnexpectedCreation)
                .resource_type(detector.resource_type_name())
                .build(),
        ))
    } else {
        Ok(None) // Resource is expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::{
        api::{
            apps::v1::{Deployment, DeploymentSpec},
            core::v1::{Container, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec},
        },
        apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta},
    };
    use std::collections::BTreeMap;

    fn create_test_configmap(
        name: &str,
        namespace: &str,
        data: BTreeMap<String, String>,
    ) -> ConfigMap {
        let mut labels = BTreeMap::new();
        labels.insert(
            "app.kubernetes.io/managed-by".to_string(),
            "vale-gateway".to_string(),
        );
        labels.insert("vale-gateway.io/managed".to_string(), "true".to_string());

        let mut annotations = BTreeMap::new();
        annotations.insert(
            "vale-gateway.io/resource-hash".to_string(),
            "test-hash".to_string(),
        );

        ConfigMap {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels),
                annotations: Some(annotations),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        }
    }

    #[test]
    fn test_drift_detector_no_drift() {
        let detector = DefaultDriftDetector::with_defaults();
        let data = BTreeMap::from([("key1".to_string(), "value1".to_string())]);
        let expected = create_test_configmap("test-cm", "test-ns", data.clone());
        let actual = create_test_configmap("test-cm", "test-ns", data);

        let result = detector
            .detect_drift(&expected, Some(&actual), "test-cm", "test-ns")
            .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_drift_detector_modified_resource() {
        let detector = DefaultDriftDetector::with_defaults();
        let expected_data = BTreeMap::from([("key1".to_string(), "value1".to_string())]);
        let actual_data = BTreeMap::from([("key1".to_string(), "value2".to_string())]);

        let expected = create_test_configmap("test-cm", "test-ns", expected_data);
        let actual = create_test_configmap("test-cm", "test-ns", actual_data);

        let result = detector
            .detect_drift(&expected, Some(&actual), "test-cm", "test-ns")
            .unwrap();

        assert!(result.is_some());
        let drift = result.unwrap();
        assert_eq!(*drift.drift_type(), DriftType::Modified);
        assert_eq!(drift.resource_name(), "test-cm");
        assert_eq!(drift.namespace(), "test-ns");
        assert!(drift.requires_restoration());
    }

    #[test]
    fn test_drift_detector_deleted_resource() {
        let detector = DefaultDriftDetector::with_defaults();
        let data = BTreeMap::from([("key1".to_string(), "value1".to_string())]);
        let expected = create_test_configmap("test-cm", "test-ns", data);

        let result = detector
            .detect_drift(&expected, None, "test-cm", "test-ns")
            .unwrap();

        assert!(result.is_some());
        let drift = result.unwrap();
        assert_eq!(*drift.drift_type(), DriftType::Deleted);
        assert_eq!(drift.resource_name(), "test-cm");
        assert_eq!(drift.namespace(), "test-ns");
        assert!(drift.actual_resource().is_none());
        assert!(drift.requires_restoration());
    }

    #[test]
    fn test_ownership_validation_success() {
        let detector = DefaultDriftDetector::with_defaults();
        let data = BTreeMap::from([("key1".to_string(), "value1".to_string())]);
        let configmap = create_test_configmap("test-cm", "test-ns", data);

        let result = detector.validate_ownership(&configmap).unwrap();
        assert!(result);
    }
}
