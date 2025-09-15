use k8s_openapi::api::core::v1::ConfigMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use vg_core::task::Builder as TaskBuilder;

use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::watchdog::configmap_watcher::ConfigMapRestorationRequest;
use crate::watchdog::data_models::DriftType;
use crate::watchdog::error::WatchdogError;

/// ConfigMap restoration coordinator that handles drift restoration
///
/// This coordinator receives restoration requests from the ConfigMap watcher
/// and coordinates with the existing ConfigMapSynchronizer to restore
/// drifted resources to their expected state.
pub struct ConfigMapRestorationCoordinator {
    /// Channel for receiving restoration requests
    restoration_rx: mpsc::UnboundedReceiver<ConfigMapRestorationRequest>,
    /// Channel for sending sync actions to the ConfigMapSynchronizer
    sync_tx: mpsc::UnboundedSender<SyncObjectAction<String, ConfigMap>>,
    /// Maximum number of restoration attempts per resource
    max_retry_attempts: u32,
    /// Exponential backoff base delay in milliseconds
    base_retry_delay_ms: u64,
}

impl ConfigMapRestorationCoordinator {
    /// Create a new ConfigMap restoration coordinator
    pub fn new(
        restoration_rx: mpsc::UnboundedReceiver<ConfigMapRestorationRequest>,
        sync_tx: mpsc::UnboundedSender<SyncObjectAction<String, ConfigMap>>,
    ) -> Self {
        Self {
            restoration_rx,
            sync_tx,
            max_retry_attempts: 3,
            base_retry_delay_ms: 1000, // 1 second
        }
    }

    /// Start the restoration coordinator
    pub fn start(mut self, task_builder: &TaskBuilder) -> Result<(), WatchdogError> {
        task_builder
            .new_task("configmap_restoration_coordinator")
            .spawn(async move {
                self.process_restoration_requests().await;
            });

        Ok(())
    }

    /// Process restoration requests from the ConfigMap watcher
    async fn process_restoration_requests(&mut self) {
        info!("ConfigMap restoration coordinator started");

        while let Some(request) = self.restoration_rx.recv().await {
            if let Err(e) = self.handle_restoration_request(request).await {
                error!("Error handling ConfigMap restoration request: {}", e);
            }
        }

        warn!("ConfigMap restoration coordinator stopped");
    }

    /// Handle a single restoration request
    async fn handle_restoration_request(
        &self,
        request: ConfigMapRestorationRequest,
    ) -> Result<(), WatchdogError> {
        info!(
            "Processing ConfigMap restoration request for {}/{}",
            request.namespace, request.name
        );

        match request.drift_type {
            DriftType::Modified => {
                self.restore_modified_configmap(&request).await?;
            }
            DriftType::Deleted => {
                self.restore_deleted_configmap(&request).await?;
            }
            DriftType::UnexpectedCreation => {
                self.handle_unexpected_configmap(&request).await?;
            }
        }

        Ok(())
    }

    /// Restore a modified ConfigMap to its expected state
    async fn restore_modified_configmap(
        &self,
        request: &ConfigMapRestorationRequest,
    ) -> Result<(), WatchdogError> {
        info!(
            "Restoring modified ConfigMap {}/{}",
            request.namespace, request.name
        );

        let object_ref = ObjectRef::builder()
            .kind("ConfigMap".to_string())
            .name(request.name.clone())
            .namespace(Some(request.namespace.clone()))
            .build();

        let sync_action = SyncObjectAction::Delete(object_ref);

        if let Err(e) = self.sync_tx.send(sync_action) {
            error!(
                "Failed to send sync action for ConfigMap {}/{}: {}",
                request.namespace, request.name, e
            );
            return Err(WatchdogError::restoration_failed(format!(
                "Failed to trigger ConfigMap reconciliation: {}",
                e
            )));
        }

        debug!(
            "Sent reconciliation request for ConfigMap {}/{}",
            request.namespace, request.name
        );

        Ok(())
    }

    /// Restore a deleted ConfigMap
    async fn restore_deleted_configmap(
        &self,
        request: &ConfigMapRestorationRequest,
    ) -> Result<(), WatchdogError> {
        warn!(
            "Restoring deleted ConfigMap {}/{}",
            request.namespace, request.name
        );

        let object_ref = ObjectRef::builder()
            .kind("ConfigMap".to_string())
            .name(request.name.clone())
            .namespace(Some(request.namespace.clone()))
            .build();

        let sync_action = SyncObjectAction::Delete(object_ref);

        if let Err(e) = self.sync_tx.send(sync_action) {
            error!(
                "Failed to send restoration action for deleted ConfigMap {}/{}: {}",
                request.namespace, request.name, e
            );
            return Err(WatchdogError::restoration_failed(format!(
                "Failed to trigger ConfigMap recreation: {}",
                e
            )));
        }

        info!(
            "Sent recreation request for deleted ConfigMap {}/{}",
            request.namespace, request.name
        );

        Ok(())
    }

    /// Handle unexpected ConfigMap creation
    async fn handle_unexpected_configmap(
        &self,
        request: &ConfigMapRestorationRequest,
    ) -> Result<(), WatchdogError> {
        warn!(
            "Handling unexpected ConfigMap creation {}/{}",
            request.namespace, request.name
        );

        debug!(
            "Unexpected ConfigMap {}/{} will be handled by normal sync process",
            request.namespace, request.name
        );

        Ok(())
    }
}

/// Create the restoration coordination channels and coordinator
pub fn create_configmap_restoration_coordinator(
    sync_tx: mpsc::UnboundedSender<SyncObjectAction<String, ConfigMap>>,
) -> (
    mpsc::UnboundedSender<ConfigMapRestorationRequest>,
    ConfigMapRestorationCoordinator,
) {
    let (restoration_tx, restoration_rx) = mpsc::unbounded_channel();
    let coordinator = ConfigMapRestorationCoordinator::new(restoration_rx, sync_tx);

    (restoration_tx, coordinator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    /// Create a test ConfigMap restoration request
    fn create_test_restoration_request(
        name: &str,
        namespace: &str,
        drift_type: DriftType,
    ) -> ConfigMapRestorationRequest {
        let mut data = BTreeMap::new();
        data.insert("config.yaml".to_string(), "test: value".to_string());

        let configmap = ConfigMap {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        };

        ConfigMapRestorationRequest {
            namespace: namespace.to_string(),
            name: name.to_string(),
            expected_configmap: configmap.clone(),
            actual_configmap: if drift_type == DriftType::Deleted {
                None
            } else {
                Some(configmap)
            },
            drift_type,
        }
    }

    #[tokio::test]
    async fn test_restoration_coordinator_creation() {
        let (_sync_tx, _sync_rx) = mpsc::unbounded_channel();
        let (restoration_tx, _coordinator) = create_configmap_restoration_coordinator(_sync_tx);

        // Verify channels were created successfully
        drop(restoration_tx);
        drop(_sync_rx);
    }

    #[tokio::test]
    async fn test_restoration_request_processing() {
        let (sync_tx, mut sync_rx) = mpsc::unbounded_channel();
        let (restoration_tx, mut coordinator) = create_configmap_restoration_coordinator(sync_tx);

        // Send a test restoration request
        let request =
            create_test_restoration_request("test-config", "default", DriftType::Modified);

        restoration_tx.send(request).unwrap();
        drop(restoration_tx);

        // Process the request
        tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            coordinator.process_restoration_requests(),
        )
        .await
        .ok();

        // Verify that a sync action was sent
        let sync_action = sync_rx.try_recv();
        assert!(sync_action.is_ok(), "Should have received a sync action");
    }
}
