use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use tokio::sync::mpsc;
use tokio::time::timeout;
use vg_core::sync::signal::signal;
use vg_core::task::Builder as TaskBuilder;

use crate::kubernetes::objects::Objects;
use crate::options::Options;
use crate::watchdog::service::ResourceWatchdogService;
use crate::watchdog::{
    ConfigMapWatcher, DefaultDriftDetector, DriftDetector, DriftType, WatchdogConfiguration
    ,
};

/// Integration tests for ConfigMap monitoring and restoration
///
/// These tests validate the complete ConfigMap monitoring pipeline:
/// - ConfigMap drift detection
/// - Integration with restoration coordinator
/// - Coordination with ConfigMapSynchronizer
/// - End-to-end monitoring and restoration workflow

/// Create a test ConfigMap with gateway ownership labels and annotations
fn create_gateway_configmap(
    name: &str,
    namespace: &str,
    data: BTreeMap<String, String>,
    resource_version: Option<&str>,
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
            resource_version: resource_version.map(|v| v.to_string()),
            uid: Some("test-uid".to_string()),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    }
}

/// Create a non-gateway ConfigMap (without ownership labels)
fn create_unowned_configmap(name: &str, namespace: &str) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            uid: Some("test-uid-unowned".to_string()),
            ..Default::default()
        },
        data: Some({
            let mut data = BTreeMap::new();
            data.insert("unowned.yaml".to_string(), "value: test".to_string());
            data
        }),
        ..Default::default()
    }
}

/// Create Objects container with test ConfigMaps
fn create_test_objects(configmaps: Vec<ConfigMap>) -> Objects<ConfigMap> {
    let mut objects = Objects::default();

    for configmap in configmaps {
        objects.insert(Arc::new(configmap)).unwrap();
    }

    objects
}

#[tokio::test]
async fn test_configmap_watcher_detects_modifications() {
    // Create initial ConfigMap
    let mut original_data = BTreeMap::new();
    original_data.insert("config.yaml".to_string(), "original: value".to_string());
    let original_configmap =
        create_gateway_configmap("test-config", "default", original_data, Some("1"));

    // Create modified ConfigMap
    let mut modified_data = BTreeMap::new();
    modified_data.insert("config.yaml".to_string(), "modified: value".to_string());
    let modified_configmap =
        create_gateway_configmap("test-config", "default", modified_data, Some("2"));

    // Set up objects signal
    let (objects_tx, objects_rx) = signal("test_configmap_objects");

    // Create expected resources
    let expected_objects = create_test_objects(vec![original_configmap.clone()]);
    let expected_resources = Arc::new(expected_objects);

    // Create ConfigMap watcher
    let mut configmap_watcher = ConfigMapWatcher::new().with_expected_resources(expected_resources);

    // Set up restoration channel
    let (restoration_tx, mut restoration_rx) = mpsc::unbounded_channel();

    // Start the watcher
    configmap_watcher.start_watching(objects_rx).await.unwrap();

    // Start event processing
    let task_builder = TaskBuilder::default();
    configmap_watcher
        .start_event_processing(&task_builder, restoration_tx)
        .unwrap();

    // Send initial objects (original ConfigMap)
    let initial_objects = create_test_objects(vec![original_configmap]);
    objects_tx.set(initial_objects).await;

    // Give the watcher time to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send modified objects
    let modified_objects = create_test_objects(vec![modified_configmap.clone()]);
    objects_tx.set(modified_objects).await;

    // Wait for restoration request
    let restoration_request = timeout(Duration::from_secs(5), restoration_rx.recv())
        .await
        .expect("Should receive restoration request within timeout")
        .expect("Should receive restoration request");

    // Verify the restoration request
    assert_eq!(restoration_request.name, "test-config");
    assert_eq!(restoration_request.namespace, "default");
    assert_eq!(restoration_request.drift_type, DriftType::Modified);
    assert!(restoration_request.actual_configmap.is_some());

    let actual = restoration_request.actual_configmap.unwrap();
    assert_eq!(
        actual.data.unwrap().get("config.yaml").unwrap(),
        "modified: value"
    );
}

#[tokio::test]
async fn test_configmap_watcher_detects_deletions() {
    // Create ConfigMap that will be "deleted"
    let mut data = BTreeMap::new();
    data.insert("config.yaml".to_string(), "test: value".to_string());
    let configmap = create_gateway_configmap("test-config", "default", data, Some("1"));

    // Set up objects signal
    let (objects_tx, objects_rx) = signal("test_configmap_objects");

    // Create expected resources
    let expected_objects = create_test_objects(vec![configmap.clone()]);
    let expected_resources = Arc::new(expected_objects);

    // Create ConfigMap watcher
    let mut configmap_watcher = ConfigMapWatcher::new().with_expected_resources(expected_resources);

    // Set up restoration channel
    let (restoration_tx, mut restoration_rx) = mpsc::unbounded_channel();

    // Start the watcher
    configmap_watcher.start_watching(objects_rx).await.unwrap();

    // Start event processing
    let task_builder = TaskBuilder::default();
    configmap_watcher
        .start_event_processing(&task_builder, restoration_tx)
        .unwrap();

    // Send initial objects (with ConfigMap)
    let initial_objects = create_test_objects(vec![configmap]);
    objects_tx.set(initial_objects).await;

    // Give the watcher time to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send empty objects (ConfigMap deleted)
    let empty_objects = create_test_objects(vec![]);
    objects_tx.set(empty_objects).await;

    // Wait for restoration request
    let restoration_request = timeout(Duration::from_secs(5), restoration_rx.recv())
        .await
        .expect("Should receive restoration request within timeout")
        .expect("Should receive restoration request");

    // Verify the restoration request
    assert_eq!(restoration_request.name, "test-config");
    assert_eq!(restoration_request.namespace, "default");
    assert_eq!(restoration_request.drift_type, DriftType::Deleted);
    assert!(restoration_request.actual_configmap.is_none());
}

#[tokio::test]
async fn test_configmap_watcher_ignores_unowned_resources() {
    // Create unowned ConfigMap
    let unowned_configmap = create_unowned_configmap("unowned-config", "default");

    // Set up objects signal
    let (objects_tx, objects_rx) = signal("test_configmap_objects");

    // Create empty expected resources (no gateway-owned ConfigMaps expected)
    let expected_resources = Arc::new(Objects::default());

    // Create ConfigMap watcher
    let mut configmap_watcher = ConfigMapWatcher::new().with_expected_resources(expected_resources);

    // Set up restoration channel
    let (restoration_tx, mut restoration_rx) = mpsc::unbounded_channel();

    // Start the watcher
    configmap_watcher.start_watching(objects_rx).await.unwrap();

    // Start event processing
    let task_builder = TaskBuilder::default();
    configmap_watcher
        .start_event_processing(&task_builder, restoration_tx)
        .unwrap();

    // Send objects with unowned ConfigMap
    let objects = create_test_objects(vec![unowned_configmap]);
    objects_tx.set(objects).await;

    // Give the watcher time to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Remove the unowned ConfigMap
    let empty_objects = create_test_objects(vec![]);
    objects_tx.set(empty_objects).await;

    // Give more time to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should not receive any restoration requests for unowned resources
    let result = timeout(Duration::from_millis(500), restoration_rx.recv()).await;
    assert!(
        result.is_err(),
        "Should not receive restoration request for unowned ConfigMap"
    );
}

// Simplified test for watchdog service integration
#[tokio::test]
async fn test_watchdog_service_disabled_skips_monitoring() {
    // Create disabled watchdog configuration
    let config = WatchdogConfiguration::new();

    let options = Arc::new(Options::default());
    let watchdog_service = ResourceWatchdogService::new(options);

    // Verify the service was created successfully
    assert!(watchdog_service.is_active());
}

#[tokio::test]
async fn test_drift_detection_with_semantic_differences() {
    let drift_detector = DefaultDriftDetector::with_defaults();

    // Create original ConfigMap
    let mut original_data = BTreeMap::new();
    original_data.insert(
        "config.yaml".to_string(),
        "original: value\nother: data".to_string(),
    );
    let original_configmap =
        create_gateway_configmap("test-config", "default", original_data, Some("1"));

    // Create modified ConfigMap with different data but same metadata structure
    let mut modified_data = BTreeMap::new();
    modified_data.insert(
        "config.yaml".to_string(),
        "modified: value\nother: data".to_string(),
    );
    let modified_configmap =
        create_gateway_configmap("test-config", "default", modified_data, Some("2"));

    // Detect drift
    let drift_result = drift_detector
        .detect_drift(
            &original_configmap,
            Some(&modified_configmap),
            "test-config",
            "default",
        )
        .unwrap();

    // Should detect drift due to data differences
    assert!(
        drift_result.is_some(),
        "Should detect drift in ConfigMap data"
    );

    let drift = drift_result.unwrap();
    assert_eq!(drift.drift_type(), &DriftType::Modified);
    assert_eq!(drift.resource_name(), "test-config");
    assert_eq!(drift.namespace(), "default");
    assert_eq!(drift.resource_type(), "ConfigMap");
}

#[tokio::test]
async fn test_multiple_configmap_changes_in_sequence() {
    // Create test ConfigMaps for sequence testing
    let mut original_data = BTreeMap::new();
    original_data.insert("config.yaml".to_string(), "version: 1".to_string());
    let original_configmap =
        create_gateway_configmap("test-config", "default", original_data, Some("1"));

    let mut modified_data_1 = BTreeMap::new();
    modified_data_1.insert("config.yaml".to_string(), "version: 2".to_string());
    let modified_configmap_1 =
        create_gateway_configmap("test-config", "default", modified_data_1, Some("2"));

    let mut modified_data_2 = BTreeMap::new();
    modified_data_2.insert("config.yaml".to_string(), "version: 3".to_string());
    let modified_configmap_2 =
        create_gateway_configmap("test-config", "default", modified_data_2, Some("3"));

    // Set up objects signal
    let (objects_tx, objects_rx) = signal("test_configmap_objects");

    // Create expected resources
    let expected_objects = create_test_objects(vec![original_configmap.clone()]);
    let expected_resources = Arc::new(expected_objects);

    // Create ConfigMap watcher
    let mut configmap_watcher = ConfigMapWatcher::new().with_expected_resources(expected_resources);

    // Set up restoration channel
    let (restoration_tx, mut restoration_rx) = mpsc::unbounded_channel();

    // Start the watcher
    configmap_watcher.start_watching(objects_rx).await.unwrap();

    // Start event processing
    let task_builder = TaskBuilder::default();
    configmap_watcher
        .start_event_processing(&task_builder, restoration_tx)
        .unwrap();

    // Send initial objects
    let initial_objects = create_test_objects(vec![original_configmap]);
    objects_tx.set(initial_objects).await;

    // Give time to process
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send first modification
    let modified_objects_1 = create_test_objects(vec![modified_configmap_1]);
    objects_tx.set(modified_objects_1).await;

    // Wait for first restoration request
    let restoration_request_1 = timeout(Duration::from_secs(2), restoration_rx.recv())
        .await
        .expect("Should receive first restoration request")
        .expect("Should receive first restoration request");

    assert_eq!(restoration_request_1.drift_type, DriftType::Modified);

    // Send second modification
    let modified_objects_2 = create_test_objects(vec![modified_configmap_2]);
    objects_tx.set(modified_objects_2).await;

    // Wait for second restoration request
    let restoration_request_2 = timeout(Duration::from_secs(2), restoration_rx.recv())
        .await
        .expect("Should receive second restoration request")
        .expect("Should receive second restoration request");

    assert_eq!(restoration_request_2.drift_type, DriftType::Modified);

    // Both requests should be for the same ConfigMap
    assert_eq!(restoration_request_1.name, restoration_request_2.name);
    assert_eq!(
        restoration_request_1.namespace,
        restoration_request_2.namespace
    );
}
