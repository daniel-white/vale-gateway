use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

use kube::{Resource, ResourceExt};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};
use vg_core::sync::signal::Receiver;

use crate::kubernetes::objects::Objects;
use crate::watchdog::{DriftDetector, WatchdogError};

/// Event types that can occur during resource watching
#[derive(Debug, Clone)]
pub enum WatchEvent<T> {
    /// A resource was added or updated
    Added(Arc<T>),
    /// A resource was modified
    Modified { old: Arc<T>, new: Arc<T> },
    /// A resource was deleted
    Deleted(Arc<T>),
    /// An error occurred during watching
    Error(WatchdogError),
}

/// Configuration for a resource watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Labels to filter resources by
    pub label_selector: Option<String>,
    /// Whether to watch all namespaces or just a specific one
    pub namespace: Option<String>,
    /// Whether to enable drift detection
    pub enable_drift_detection: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            label_selector: None,
            namespace: None,
            enable_drift_detection: true,
        }
    }
}

/// Generic resource watcher that can monitor any Kubernetes resource type
///
/// This watcher integrates with the existing watch_objects macro pattern
/// and provides drift detection capabilities through event handling.
/// All data structures are immutable as required.
pub struct ResourceWatcher<T>
where
    T: Resource + Clone + Debug + Serialize + DeserializeOwned + Send + Sync + PartialEq + 'static,
    T::DynamicType: Default,
{
    /// Configuration for this watcher
    config: WatcherConfig,
    /// Drift detector for this resource type
    drift_detector: Option<Arc<dyn DriftDetector<T> + Send + Sync>>,
    /// Expected state of resources (managed by the gateway)
    expected_resources: Arc<Objects<T>>,
    /// Channel for sending watch events
    event_sender: mpsc::UnboundedSender<WatchEvent<T>>,
    /// Channel for receiving watch events
    event_receiver: mpsc::UnboundedReceiver<WatchEvent<T>>,
    /// Phantom data to hold the resource type
    _phantom: PhantomData<T>,
}

impl<T> ResourceWatcher<T>
where
    T: Resource + Clone + Debug + Serialize + DeserializeOwned + Send + Sync + PartialEq + 'static,
    T::DynamicType: Default,
{
    /// Create a new resource watcher with the given configuration
    pub fn new(config: WatcherConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            config,
            drift_detector: None,
            expected_resources: Arc::new(Objects::default()),
            event_sender,
            event_receiver,
            _phantom: PhantomData,
        }
    }

    /// Set the drift detector for this watcher
    pub fn with_drift_detector(
        mut self,
        drift_detector: Arc<dyn DriftDetector<T> + Send + Sync>,
    ) -> Self {
        self.drift_detector = Some(drift_detector);
        self
    }

    /// Set the expected resources for drift detection
    pub fn with_expected_resources(mut self, expected_resources: Arc<Objects<T>>) -> Self {
        self.expected_resources = expected_resources;
        self
    }

    /// Get the event receiver for processing watch events
    pub fn event_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<WatchEvent<T>> {
        &mut self.event_receiver
    }

    /// Start watching resources using the existing watch_objects pattern
    ///
    /// This method integrates with the existing Kubernetes watching infrastructure
    /// and provides an event-driven interface for drift detection.
    pub async fn start_watching(
        &self,
        objects_rx: Receiver<Objects<T>>,
    ) -> Result<(), WatchdogError> {
        let event_sender = self.event_sender.clone();
        let config = self.config.clone();
        let drift_detector = self.drift_detector.clone();
        let expected_resources = self.expected_resources.clone();

        // Spawn a task to monitor the objects receiver and generate events
        tokio::spawn(async move {
            let mut previous_objects: Option<Objects<T>> = None;

            loop {
                match objects_rx.get().await.as_ref() {
                    Some(current_objects) => {
                        if let Err(e) = Self::process_objects_update(
                            &previous_objects,
                            current_objects,
                            &event_sender,
                            &config,
                            &drift_detector,
                            &expected_resources,
                        )
                        .await
                        {
                            error!("Error processing objects update: {}", e);
                            let _ = event_sender.send(WatchEvent::Error(e));
                        }

                        previous_objects = Some(current_objects.clone());
                    }
                    None => {
                        debug!("Objects receiver returned None, continuing to wait");
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Process updates from the objects receiver and generate appropriate events
    async fn process_objects_update(
        previous_objects: &Option<Objects<T>>,
        current_objects: &Objects<T>,
        event_sender: &mpsc::UnboundedSender<WatchEvent<T>>,
        config: &WatcherConfig,
        drift_detector: &Option<Arc<dyn DriftDetector<T> + Send + Sync>>,
        expected_resources: &Arc<Objects<T>>,
    ) -> Result<(), WatchdogError> {
        match previous_objects {
            None => {
                // First time seeing objects, treat all as added
                for (_, _, object) in current_objects.iter() {
                    let _ = event_sender.send(WatchEvent::Added(object.clone()));

                    // Check for drift if enabled
                    if config.enable_drift_detection {
                        Self::check_drift_for_object(
                            &object,
                            drift_detector,
                            expected_resources,
                            event_sender,
                        )
                        .await?;
                    }
                }
            }
            Some(prev) => {
                // Compare with previous state to detect changes
                Self::detect_changes(
                    prev,
                    current_objects,
                    event_sender,
                    config,
                    drift_detector,
                    expected_resources,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Detect changes between previous and current object states
    async fn detect_changes(
        previous: &Objects<T>,
        current: &Objects<T>,
        event_sender: &mpsc::UnboundedSender<WatchEvent<T>>,
        config: &WatcherConfig,
        drift_detector: &Option<Arc<dyn DriftDetector<T> + Send + Sync>>,
        expected_resources: &Arc<Objects<T>>,
    ) -> Result<(), WatchdogError> {
        // Find added and modified objects
        for (obj_ref, _, current_obj) in current.iter() {
            match previous.get_by_ref(&obj_ref) {
                Some(prev_obj) => {
                    // Object existed before, check if modified
                    if !Self::objects_equal(&prev_obj, &current_obj) {
                        let _ = event_sender.send(WatchEvent::Modified {
                            old: prev_obj.clone(),
                            new: current_obj.clone(),
                        });
                    }
                }
                None => {
                    // New object
                    let _ = event_sender.send(WatchEvent::Added(current_obj.clone()));
                }
            }

            // Check for drift if enabled
            if config.enable_drift_detection {
                Self::check_drift_for_object(
                    &current_obj,
                    drift_detector,
                    expected_resources,
                    event_sender,
                )
                .await?;
            }
        }

        // Find deleted objects
        for (obj_ref, _, prev_obj) in previous.iter() {
            if current.get_by_ref(&obj_ref).is_none() {
                let _ = event_sender.send(WatchEvent::Deleted(prev_obj.clone()));
            }
        }

        Ok(())
    }

    /// Check for drift in a specific object
    async fn check_drift_for_object(
        actual_object: &Arc<T>,
        drift_detector: &Option<Arc<dyn DriftDetector<T> + Send + Sync>>,
        expected_resources: &Arc<Objects<T>>,
        event_sender: &mpsc::UnboundedSender<WatchEvent<T>>,
    ) -> Result<(), WatchdogError> {
        if let Some(detector) = drift_detector {
            let obj_ref = Self::get_object_ref(actual_object)?;
            let expected = expected_resources.get_by_ref(&obj_ref);

            let resource_name = actual_object.name_any();
            let namespace = actual_object.namespace().unwrap_or_default();

            // Only perform drift detection if we have an expected resource to compare against
            if let Some(expected_resource) = expected {
                match detector.detect_drift(
                    expected_resource.as_ref(),
                    Some(actual_object.as_ref()),
                    &resource_name,
                    &namespace,
                ) {
                    Ok(Some(drift)) => {
                        warn!("Detected drift: {}", drift.description());
                        // Drift events would be handled by restoration coordinator
                        // This is just for logging and monitoring
                    }
                    Ok(None) => {
                        debug!("No drift detected for {}/{}", namespace, resource_name);
                    }
                    Err(e) => {
                        error!(
                            "Error detecting drift for {}/{}: {}",
                            namespace, resource_name, e
                        );
                        let _ = event_sender.send(WatchEvent::Error(e));
                    }
                }
            } else {
                debug!(
                    "No expected resource found for {}/{}, skipping drift detection",
                    namespace, resource_name
                );
            }
        }

        Ok(())
    }

    /// Get an object reference for the given object
    fn get_object_ref(
        object: &Arc<T>,
    ) -> Result<crate::kubernetes::objects::ObjectRef, WatchdogError> {
        use crate::kubernetes::objects::ObjectRef;

        let name = object.name_any();
        if name.is_empty() {
            return Err(WatchdogError::invalid_resource("Object missing name"));
        }

        Ok(ObjectRef::builder()
            .kind(T::kind(&T::DynamicType::default()).to_string())
            .name(name)
            .namespace(object.namespace())
            .build())
    }

    /// Check if two objects are equal (simplified comparison)
    fn objects_equal(obj1: &Arc<T>, obj2: &Arc<T>) -> bool {
        // For now, compare resource versions as a simple equality check
        // In a more sophisticated implementation, this could do deep comparison
        // of specific fields while ignoring metadata like timestamps
        obj1.resource_version() == obj2.resource_version()
    }
}

/// Builder for creating ResourceWatcher instances with configuration
pub struct ResourceWatcherBuilder<T>
where
    T: Resource + Clone + Debug + Serialize + DeserializeOwned + Send + Sync + PartialEq + 'static,
    T::DynamicType: Default,
{
    config: WatcherConfig,
    drift_detector: Option<Arc<dyn DriftDetector<T> + Send + Sync>>,
    expected_resources: Option<Arc<Objects<T>>>,
    _phantom: PhantomData<T>,
}

impl<T> ResourceWatcherBuilder<T>
where
    T: Resource + Clone + Debug + Serialize + DeserializeOwned + Send + Sync + PartialEq + 'static,
    T::DynamicType: Default,
{
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: WatcherConfig::default(),
            drift_detector: None,
            expected_resources: None,
            _phantom: PhantomData,
        }
    }

    /// Set the label selector for filtering resources
    pub fn with_label_selector<S: Into<String>>(mut self, selector: S) -> Self {
        self.config.label_selector = Some(selector.into());
        self
    }

    /// Set the namespace to watch (None means all namespaces)
    pub fn with_namespace<S: Into<String>>(mut self, namespace: Option<S>) -> Self {
        self.config.namespace = namespace.map(|n| n.into());
        self
    }

    /// Enable or disable drift detection
    pub fn with_drift_detection(mut self, enabled: bool) -> Self {
        self.config.enable_drift_detection = enabled;
        self
    }

    /// Set the drift detector
    pub fn with_drift_detector(
        mut self,
        detector: Arc<dyn DriftDetector<T> + Send + Sync>,
    ) -> Self {
        self.drift_detector = Some(detector);
        self
    }

    /// Set the expected resources for drift detection
    pub fn with_expected_resources(mut self, resources: Arc<Objects<T>>) -> Self {
        self.expected_resources = Some(resources);
        self
    }

    /// Build the ResourceWatcher
    pub fn build(self) -> ResourceWatcher<T> {
        let mut watcher = ResourceWatcher::new(self.config);

        if let Some(detector) = self.drift_detector {
            watcher = watcher.with_drift_detector(detector);
        }

        if let Some(resources) = self.expected_resources {
            watcher = watcher.with_expected_resources(resources);
        }

        watcher
    }
}

impl<T> Default for ResourceWatcherBuilder<T>
where
    T: Resource + Clone + Debug + Serialize + DeserializeOwned + Send + Sync + PartialEq + 'static,
    T::DynamicType: Default,
{
    fn default() -> Self {
        Self::new()
    }
}
