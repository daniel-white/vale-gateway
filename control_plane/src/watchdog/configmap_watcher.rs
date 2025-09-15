use std::sync::Arc;

use k8s_openapi::api::core::v1::ConfigMap;
use kube::ResourceExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;

use crate::kubernetes::objects::Objects;
use crate::watchdog::data_models::DriftType;
use crate::watchdog::drift_detector::DriftDetector;
use crate::watchdog::error::WatchdogError;
use crate::watchdog::{DefaultDriftDetector, ResourceWatcher, WatchEvent, WatcherConfig};

/// ConfigMap-specific watcher that monitors ConfigMap resources for drift
pub struct ConfigMapWatcher {
    /// The underlying generic resource watcher
    watcher: ResourceWatcher<ConfigMap>,
    /// ConfigMap-specific drift detector
    drift_detector: Arc<dyn DriftDetector<ConfigMap> + Send + Sync>,
}

impl ConfigMapWatcher {
    /// Create a new ConfigMap watcher with default configuration
    pub fn new() -> Self {
        let config = WatcherConfig {
            label_selector: Some("app.kubernetes.io/managed-by=vale-gateway".to_string()),
            namespace: None, // Watch all namespaces
            enable_drift_detection: true,
        };

        let drift_detector = Arc::new(DefaultDriftDetector::with_defaults());
        let watcher = ResourceWatcher::new(config).with_drift_detector(drift_detector.clone());

        Self {
            watcher,
            drift_detector,
        }
    }

    /// Set the expected ConfigMap resources for drift detection
    pub fn with_expected_resources(mut self, expected_resources: Arc<Objects<ConfigMap>>) -> Self {
        self.watcher = self.watcher.with_expected_resources(expected_resources);
        self
    }

    /// Start the ConfigMap watcher with the given objects receiver
    pub async fn start_watching(
        &self,
        objects_rx: Receiver<Objects<ConfigMap>>,
    ) -> Result<(), WatchdogError> {
        info!("Starting ConfigMap monitoring");
        self.watcher.start_watching(objects_rx).await?;
        debug!("ConfigMap watcher started successfully");
        Ok(())
    }

    /// Start processing ConfigMap events for drift detection and restoration
    pub fn start_event_processing(
        &mut self,
        task_builder: &TaskBuilder,
        restoration_tx: mpsc::UnboundedSender<ConfigMapRestorationRequest>,
    ) -> Result<(), WatchdogError> {
        let event_receiver = self.watcher.take_event_receiver();
        let drift_detector = self.drift_detector.clone();

        task_builder
            .new_task("configmap_event_processor")
            .spawn(async move {
                Self::process_events(event_receiver, drift_detector, restoration_tx).await;
            });

        Ok(())
    }

    /// Process ConfigMap watch events and coordinate restoration when drift is detected
    async fn process_events(
        mut event_receiver: mpsc::UnboundedReceiver<WatchEvent<ConfigMap>>,
        drift_detector: Arc<dyn DriftDetector<ConfigMap> + Send + Sync>,
        restoration_tx: mpsc::UnboundedSender<ConfigMapRestorationRequest>,
    ) {
        info!("ConfigMap event processor started");

        while let Some(event) = event_receiver.recv().await {
            match event {
                WatchEvent::Added(configmap) => {
                    debug!(
                        "ConfigMap added: {}/{}",
                        configmap.namespace().unwrap_or_default(),
                        configmap.name_any()
                    );

                    if let Err(e) =
                        Self::handle_added_configmap(&configmap, &drift_detector, &restoration_tx)
                            .await
                    {
                        error!("Error handling added ConfigMap: {}", e);
                    }
                }
                WatchEvent::Modified { old, new } => {
                    debug!(
                        "ConfigMap modified: {}/{}",
                        new.namespace().unwrap_or_default(),
                        new.name_any()
                    );

                    if let Err(e) = Self::handle_modified_configmap(
                        &old,
                        &new,
                        &drift_detector,
                        &restoration_tx,
                    )
                    .await
                    {
                        error!("Error handling modified ConfigMap: {}", e);
                    }
                }
                WatchEvent::Deleted(configmap) => {
                    warn!(
                        "ConfigMap deleted: {}/{}",
                        configmap.namespace().unwrap_or_default(),
                        configmap.name_any()
                    );

                    if let Err(e) =
                        Self::handle_deleted_configmap(&configmap, &drift_detector, &restoration_tx)
                            .await
                    {
                        error!("Error handling deleted ConfigMap: {}", e);
                    }
                }
                WatchEvent::Error(err) => {
                    error!("ConfigMap watch error: {}", err);
                }
            }
        }

        warn!("ConfigMap event processor stopped");
    }

    /// Handle a newly added ConfigMap
    async fn handle_added_configmap(
        configmap: &Arc<ConfigMap>,
        drift_detector: &Arc<dyn DriftDetector<ConfigMap> + Send + Sync>,
        _restoration_tx: &mpsc::UnboundedSender<ConfigMapRestorationRequest>,
    ) -> Result<(), WatchdogError> {
        if !drift_detector.validate_ownership(configmap)? {
            debug!(
                "ConfigMap {}/{} is not owned by gateway, ignoring",
                configmap.namespace().unwrap_or_default(),
                configmap.name_any()
            );
            return Ok(());
        }

        info!(
            "Gateway-owned ConfigMap {}/{} was added - monitoring for drift",
            configmap.namespace().unwrap_or_default(),
            configmap.name_any()
        );

        Ok(())
    }

    /// Handle a modified ConfigMap and check for drift
    async fn handle_modified_configmap(
        old: &Arc<ConfigMap>,
        new: &Arc<ConfigMap>,
        drift_detector: &Arc<dyn DriftDetector<ConfigMap> + Send + Sync>,
        restoration_tx: &mpsc::UnboundedSender<ConfigMapRestorationRequest>,
    ) -> Result<(), WatchdogError> {
        if !drift_detector.validate_ownership(new)? {
            return Ok(());
        }

        let resource_name = new.name_any();
        let namespace = new.namespace().unwrap_or_default();

        if let Ok(Some(drift)) = drift_detector.detect_drift(
            old.as_ref(),
            Some(new.as_ref()),
            &resource_name,
            &namespace,
        ) {
            warn!(
                "Detected ConfigMap drift: {} - requesting restoration",
                drift.description()
            );

            let restoration_request = ConfigMapRestorationRequest {
                namespace: namespace.to_string(),
                name: resource_name.to_string(),
                expected_configmap: old.as_ref().clone(),
                actual_configmap: Some(new.as_ref().clone()),
                drift_type: drift.drift_type().clone(),
            };

            if let Err(e) = restoration_tx.send(restoration_request) {
                error!("Failed to send ConfigMap restoration request: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a deleted ConfigMap
    async fn handle_deleted_configmap(
        configmap: &Arc<ConfigMap>,
        drift_detector: &Arc<dyn DriftDetector<ConfigMap> + Send + Sync>,
        restoration_tx: &mpsc::UnboundedSender<ConfigMapRestorationRequest>,
    ) -> Result<(), WatchdogError> {
        if !drift_detector.validate_ownership(configmap)? {
            return Ok(());
        }

        let resource_name = configmap.name_any();
        let namespace = configmap.namespace().unwrap_or_default();

        warn!(
            "Gateway-owned ConfigMap {}/{} was deleted - requesting restoration",
            namespace, resource_name
        );

        let restoration_request = ConfigMapRestorationRequest {
            namespace: namespace.to_string(),
            name: resource_name.to_string(),
            expected_configmap: configmap.as_ref().clone(),
            actual_configmap: None,
            drift_type: DriftType::Deleted,
        };

        if let Err(e) = restoration_tx.send(restoration_request) {
            error!("Failed to send ConfigMap restoration request: {}", e);
        }

        Ok(())
    }
}

impl Default for ConfigMapWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Request for ConfigMap restoration to coordinate with the ConfigMapSynchronizer
#[derive(Debug, Clone)]
pub struct ConfigMapRestorationRequest {
    /// Namespace of the ConfigMap
    pub namespace: String,
    /// Name of the ConfigMap
    pub name: String,
    /// Expected ConfigMap state
    pub expected_configmap: ConfigMap,
    /// Actual ConfigMap state (None if deleted)
    pub actual_configmap: Option<ConfigMap>,
    /// Type of drift detected
    pub drift_type: DriftType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    /// Create a test ConfigMap with gateway ownership labels
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

    #[tokio::test]
    async fn test_configmap_watcher_creation() {
        let watcher = ConfigMapWatcher::new();
        assert!(watcher.drift_detector.resource_type_name() == "ConfigMap");
    }

    #[tokio::test]
    async fn test_drift_detection_ownership_validation() {
        let drift_detector = DefaultDriftDetector::with_defaults();

        let mut data = BTreeMap::new();
        data.insert("config.yaml".to_string(), "test: value".to_string());
        let owned_configmap = create_test_configmap("test-config", "default", data);

        let is_owned = drift_detector.validate_ownership(&owned_configmap).unwrap();
        assert!(is_owned, "Should recognize gateway-owned ConfigMap");

        let unowned_configmap = ConfigMap {
            metadata: ObjectMeta {
                name: Some("unowned-config".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let is_not_owned = drift_detector
            .validate_ownership(&unowned_configmap)
            .unwrap();
        assert!(!is_not_owned, "Should not recognize non-gateway ConfigMap");
    }
}
