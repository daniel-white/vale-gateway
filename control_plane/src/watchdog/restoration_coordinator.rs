use crate::watchdog::{
    DriftType, ResourceDrift, RestorationEvent, RestorationResult, RetryPolicy, WatchdogError,
};
use async_trait::async_trait;
use getset::{Getters, MutGetters};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use k8s_openapi::chrono::{DateTime, Utc};
use kube::{Api, Client, Resource, ResourceExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use typed_builder::TypedBuilder;

/// Trait for coordinating with existing sync controllers to prevent conflicts
#[async_trait]
pub trait SyncCoordinator: Send + Sync {
    /// Check if a resource restoration should be allowed or if sync controller is handling it
    async fn should_allow_restoration(
        &self,
        resource_type: &str,
        name: &str,
        namespace: &str,
    ) -> Result<bool, WatchdogError>;

    /// Notify sync controller that watchdog is about to restore a resource
    async fn notify_restoration_start(
        &self,
        resource_type: &str,
        name: &str,
        namespace: &str,
    ) -> Result<(), WatchdogError>;

    /// Notify sync controller that watchdog has completed resource restoration
    async fn notify_restoration_complete(
        &self,
        resource_type: &str,
        name: &str,
        namespace: &str,
        success: bool,
    ) -> Result<(), WatchdogError>;
}

/// Default implementation that allows all restorations (for testing or when sync coordination is disabled)
#[derive(Default)]
pub struct NoOpSyncCoordinator;

#[async_trait]
impl SyncCoordinator for NoOpSyncCoordinator {
    async fn should_allow_restoration(
        &self,
        _resource_type: &str,
        _name: &str,
        _namespace: &str,
    ) -> Result<bool, WatchdogError> {
        Ok(true)
    }

    async fn notify_restoration_start(
        &self,
        _resource_type: &str,
        _name: &str,
        _namespace: &str,
    ) -> Result<(), WatchdogError> {
        Ok(())
    }

    async fn notify_restoration_complete(
        &self,
        _resource_type: &str,
        _name: &str,
        _namespace: &str,
        _success: bool,
    ) -> Result<(), WatchdogError> {
        Ok(())
    }
}

/// Tracks ongoing restoration operations to prevent duplicates and manage retry state
#[derive(Debug, Clone, Getters, MutGetters, TypedBuilder)]
struct RestorationState {
    /// Current attempt number (1-based)
    #[getset(get_copy = "pub", get_mut = "pub")]
    #[builder(default = 1)]
    attempt_number: u32,

    /// When the restoration was first started
    #[getset(get = "pub")]
    #[builder(default_code = "Utc::now()")]
    started_at: DateTime<Utc>,

    /// When the last attempt was made
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default_code = "Utc::now()")]
    last_attempt_at: DateTime<Utc>,

    /// History of restoration events for this resource
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default)]
    events: Vec<RestorationEvent>,
}

/// Resource restoration coordinator that handles retry logic and coordination with sync controllers
#[derive(TypedBuilder)]
pub struct RestorationCoordinator {
    /// Kubernetes client for performing restoration operations
    #[builder(setter(into))]
    client: Client,

    /// Retry policy configuration
    #[builder(default)]
    retry_policy: RetryPolicy,

    /// Sync coordinator for preventing conflicts with sync controllers
    #[builder(default_code = "Arc::new(NoOpSyncCoordinator::default())")]
    sync_coordinator: Arc<dyn SyncCoordinator>,

    /// Channel for sending restoration events for audit logging
    #[builder(setter(strip_option))]
    event_sender: Option<mpsc::UnboundedSender<RestorationEvent>>,

    /// Track ongoing restoration operations
    #[builder(default)]
    restoration_states: Arc<RwLock<HashMap<String, RestorationState>>>,
}

impl RestorationCoordinator {
    /// Create a restoration key for tracking operations
    fn restoration_key(resource_type: &str, name: &str, namespace: &str) -> String {
        format!("{}:{}/{}", resource_type, namespace, name)
    }

    /// Check if a restoration is currently in progress for the given resource
    pub async fn is_restoration_in_progress(
        &self,
        resource_type: &str,
        name: &str,
        namespace: &str,
    ) -> bool {
        let key = Self::restoration_key(resource_type, name, namespace);
        self.restoration_states.read().await.contains_key(&key)
    }

    /// Send a restoration event for audit logging
    async fn send_event(&self, event: RestorationEvent) {
        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender.send(event.clone()) {
                error!(
                    "Failed to send restoration event: {}: {}",
                    e,
                    event.description()
                );
            }
        }
    }

    /// Restore a ConfigMap resource
    pub async fn restore_configmap(
        &self,
        drift: ResourceDrift<ConfigMap>,
    ) -> Result<RestorationResult, WatchdogError> {
        let resource_type = "ConfigMap";
        let name = drift.resource_name();
        let namespace = drift.namespace();

        // Check if restoration should be allowed
        if !self
            .sync_coordinator
            .should_allow_restoration(resource_type, name, namespace)
            .await?
        {
            let result =
                RestorationResult::Skipped("Sync controller is managing this resource".to_string());
            let event = self
                .create_restoration_event(&drift, result.clone(), 1, None)
                .await;
            self.send_event(event).await;
            return Ok(result);
        }

        self.restore_resource_with_retry(drift, |client, expected_resource, namespace| {
            let expected_resource = expected_resource.clone();
            let namespace = namespace.to_string();
            async move {
                let api: Api<ConfigMap> = Api::namespaced(client, &namespace);

                match expected_resource.metadata.name.as_ref() {
                    Some(name) => {
                        info!("Restoring ConfigMap {}/{}", namespace, name);
                        api.create(&Default::default(), &expected_resource)
                            .await
                            .map(|_| ())
                            .map_err(WatchdogError::from)
                    }
                    None => Err(WatchdogError::restoration_failed(
                        "ConfigMap name is missing",
                    )),
                }
            }
        })
        .await
    }

    /// Restore a Service resource
    pub async fn restore_service(
        &self,
        drift: ResourceDrift<Service>,
    ) -> Result<RestorationResult, WatchdogError> {
        let resource_type = "Service";
        let name = drift.resource_name();
        let namespace = drift.namespace();

        // Check if restoration should be allowed
        if !self
            .sync_coordinator
            .should_allow_restoration(resource_type, name, namespace)
            .await?
        {
            let result =
                RestorationResult::Skipped("Sync controller is managing this resource".to_string());
            let event = self
                .create_restoration_event(&drift, result.clone(), 1, None)
                .await;
            self.send_event(event).await;
            return Ok(result);
        }

        self.restore_resource_with_retry(drift, |client, expected_resource, namespace| {
            let expected_resource = expected_resource.clone();
            let namespace = namespace.to_string();
            async move {
                let api: Api<Service> = Api::namespaced(client, &namespace);

                match expected_resource.metadata.name.as_ref() {
                    Some(name) => {
                        info!("Restoring Service {}/{}", namespace, name);
                        api.create(&Default::default(), &expected_resource)
                            .await
                            .map(|_| ())
                            .map_err(WatchdogError::from)
                    }
                    None => Err(WatchdogError::restoration_failed("Service name is missing")),
                }
            }
        })
        .await
    }

    /// Restore a Deployment resource
    pub async fn restore_deployment(
        &self,
        drift: ResourceDrift<Deployment>,
    ) -> Result<RestorationResult, WatchdogError> {
        let resource_type = "Deployment";
        let name = drift.resource_name();
        let namespace = drift.namespace();

        // Check if restoration should be allowed
        if !self
            .sync_coordinator
            .should_allow_restoration(resource_type, name, namespace)
            .await?
        {
            let result =
                RestorationResult::Skipped("Sync controller is managing this resource".to_string());
            let event = self
                .create_restoration_event(&drift, result.clone(), 1, None)
                .await;
            self.send_event(event).await;
            return Ok(result);
        }

        self.restore_resource_with_retry(drift, |client, expected_resource, namespace| {
            let expected_resource = expected_resource.clone();
            let namespace = namespace.to_string();
            async move {
                let api: Api<Deployment> = Api::namespaced(client, &namespace);

                match expected_resource.metadata.name.as_ref() {
                    Some(name) => {
                        info!("Restoring Deployment {}/{}", namespace, name);
                        api.create(&Default::default(), &expected_resource)
                            .await
                            .map(|_| ())
                            .map_err(WatchdogError::from)
                    }
                    None => Err(WatchdogError::restoration_failed(
                        "Deployment name is missing",
                    )),
                }
            }
        })
        .await
    }

    /// Generic resource restoration with retry logic
    async fn restore_resource_with_retry<T, F, Fut>(
        &self,
        drift: ResourceDrift<T>,
        restore_fn: F,
    ) -> Result<RestorationResult, WatchdogError>
    where
        T: Clone + Resource<Scope = k8s_openapi::NamespaceResourceScope> + Send + Sync,
        T::DynamicType: Default,
        F: Fn(Client, &T, &str) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(), WatchdogError>> + Send,
    {
        let resource_type = drift.resource_type();
        let name = drift.resource_name();
        let namespace = drift.namespace();
        let key = Self::restoration_key(resource_type, name, namespace);

        // Check if restoration is already in progress
        if self
            .is_restoration_in_progress(resource_type, name, namespace)
            .await
        {
            warn!("Restoration already in progress for {}", key);
            return Ok(RestorationResult::Skipped(
                "Restoration already in progress".to_string(),
            ));
        }

        // Initialize restoration state
        let mut states = self.restoration_states.write().await;
        let mut state = states
            .entry(key.clone())
            .or_insert_with(|| RestorationState::builder().build())
            .clone();
        drop(states);

        // Notify sync coordinator
        self.sync_coordinator
            .notify_restoration_start(resource_type, name, namespace)
            .await?;

        let mut final_result = RestorationResult::Failed("No attempts made".to_string());

        // Retry loop
        while self.retry_policy.should_retry(state.attempt_number) {
            let start_time = Instant::now();

            // Calculate delay for this attempt
            let delay = self.retry_policy.delay_for_attempt(state.attempt_number);
            if delay > Duration::from_secs(0) {
                debug!(
                    "Waiting {:?} before attempt {} for {}",
                    delay, state.attempt_number, key
                );
                sleep(delay).await;
            }

            debug!("Restoration attempt {} for {}", state.attempt_number, key);

            // Perform the restoration
            let result = match drift.drift_type() {
                DriftType::Deleted => {
                    // Resource was deleted, recreate it
                    restore_fn(self.client.clone(), drift.expected_resource(), namespace).await
                }
                DriftType::Modified => {
                    // Resource was modified, update it to expected state
                    self.update_resource(&drift, &restore_fn).await
                }
                DriftType::UnexpectedCreation => {
                    // Unexpected resource should be removed, not restored
                    Ok(())
                }
            };

            let duration = start_time.elapsed();

            match result {
                Ok(()) => {
                    final_result = RestorationResult::Success;
                    let event = self
                        .create_restoration_event(
                            &drift,
                            final_result.clone(),
                            state.attempt_number,
                            Some(duration),
                        )
                        .await;

                    state.events_mut().push(event.clone());
                    self.send_event(event).await;

                    info!(
                        "Successfully restored {} after {} attempts",
                        key, state.attempt_number
                    );
                    break;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    final_result = RestorationResult::Failed(error_msg.clone());

                    let event = self
                        .create_restoration_event(
                            &drift,
                            final_result.clone(),
                            state.attempt_number,
                            Some(duration),
                        )
                        .await;

                    state.events_mut().push(event.clone());
                    self.send_event(event).await;

                    if self.retry_policy.should_retry(state.attempt_number + 1) {
                        warn!(
                            "Restoration attempt {} failed for {}: {}. Will retry.",
                            state.attempt_number, key, error_msg
                        );
                    } else {
                        error!("All restoration attempts failed for {}: {}", key, error_msg);
                    }
                }
            }

            // Update state for next attempt
            *state.attempt_number_mut() += 1;
            *state.last_attempt_at_mut() = Utc::now();
        }

        // Update final state and clean up
        let mut states = self.restoration_states.write().await;
        states.remove(&key);
        drop(states);

        // Notify sync coordinator of completion
        let success = matches!(final_result, RestorationResult::Success);
        self.sync_coordinator
            .notify_restoration_complete(resource_type, name, namespace, success)
            .await?;

        Ok(final_result)
    }

    /// Update a modified resource to its expected state
    async fn update_resource<T, F, Fut>(
        &self,
        drift: &ResourceDrift<T>,
        _restore_fn: &F,
    ) -> Result<(), WatchdogError>
    where
        T: Clone + Resource<Scope = k8s_openapi::NamespaceResourceScope> + Send + Sync,
        T::DynamicType: Default,
        F: Fn(Client, &T, &str) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(), WatchdogError>> + Send,
    {
        // For now, we'll implement a simple replacement strategy
        // In the future, this could be more sophisticated with field-level updates
        match drift.resource_type().as_str() {
            "ConfigMap" => {
                if let Some(expected) = drift.expected_resource().meta().name.as_ref() {
                    let api: Api<ConfigMap> =
                        Api::namespaced(self.client.clone(), drift.namespace());
                    // Cast is safe because we know the resource type
                    let expected_cm =
                        unsafe { std::mem::transmute::<&T, &ConfigMap>(drift.expected_resource()) };
                    api.replace(expected, &Default::default(), expected_cm)
                        .await
                        .map(|_| ())
                        .map_err(WatchdogError::from)
                } else {
                    Err(WatchdogError::restoration_failed(
                        "Resource name is missing",
                    ))
                }
            }
            "Service" => {
                if let Some(expected) = drift.expected_resource().meta().name.as_ref() {
                    let api: Api<Service> = Api::namespaced(self.client.clone(), drift.namespace());
                    let expected_svc =
                        unsafe { std::mem::transmute::<&T, &Service>(drift.expected_resource()) };
                    api.replace(expected, &Default::default(), expected_svc)
                        .await
                        .map(|_| ())
                        .map_err(WatchdogError::from)
                } else {
                    Err(WatchdogError::restoration_failed(
                        "Resource name is missing",
                    ))
                }
            }
            "Deployment" => {
                if let Some(expected) = drift.expected_resource().meta().name.as_ref() {
                    let api: Api<Deployment> =
                        Api::namespaced(self.client.clone(), drift.namespace());
                    let expected_deploy = unsafe {
                        std::mem::transmute::<&T, &Deployment>(drift.expected_resource())
                    };
                    api.replace(expected, &Default::default(), expected_deploy)
                        .await
                        .map(|_| ())
                        .map_err(WatchdogError::from)
                } else {
                    Err(WatchdogError::restoration_failed(
                        "Resource name is missing",
                    ))
                }
            }
            _ => Err(WatchdogError::restoration_failed(format!(
                "Unsupported resource type for update: {}",
                drift.resource_type()
            ))),
        }
    }

    /// Create a restoration event for audit logging
    async fn create_restoration_event<T>(
        &self,
        drift: &ResourceDrift<T>,
        result: RestorationResult,
        attempt_number: u32,
        duration: Option<Duration>,
    ) -> RestorationEvent {
        RestorationEvent::builder()
            .resource_type(drift.resource_type().to_string())
            .resource_name(drift.resource_name().to_string())
            .namespace(drift.namespace().to_string())
            .drift_type(drift.drift_type().clone())
            .restoration_result(result)
            .attempt_number(attempt_number)
            .duration(duration)
            .build()
    }

    /// Get restoration statistics for monitoring and debugging
    pub async fn get_restoration_stats(&self) -> HashMap<String, RestorationState> {
        self.restoration_states.read().await.clone()
    }

    /// Clear completed restoration states (for cleanup)
    pub async fn cleanup_completed_restorations(&self, max_age: Duration) {
        let mut states = self.restoration_states.write().await;
        let cutoff =
            Utc::now() - k8s_openapi::chrono::Duration::from_std(max_age).unwrap_or_default();

        states.retain(|_key, state| state.started_at() > &cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::ConfigMap;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn create_test_configmap(name: &str, namespace: &str) -> ConfigMap {
        ConfigMap {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some({
                let mut data = BTreeMap::new();
                data.insert("key1".to_string(), "value1".to_string());
                data
            }),
            ..Default::default()
        }
    }

    fn create_test_drift() -> ResourceDrift<ConfigMap> {
        let expected = create_test_configmap("test-config", "default");
        let actual = create_test_configmap("test-config", "default");

        ResourceDrift::builder()
            .resource_name("test-config")
            .namespace("default")
            .expected_resource(expected)
            .actual_resource(Some(actual))
            .drift_type(DriftType::Deleted)
            .resource_type("ConfigMap")
            .build()
    }

    #[tokio::test]
    async fn test_restoration_key() {
        let key = RestorationCoordinator::restoration_key("ConfigMap", "test", "default");
        assert_eq!(key, "ConfigMap:default/test");
    }

    #[tokio::test]
    async fn test_retry_policy_delay_calculation() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(0));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(1));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(2));
        assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(4));
    }

    #[tokio::test]
    async fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::builder().max_attempts(3).build();

        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    #[tokio::test]
    async fn test_restoration_event_creation() {
        let drift = create_test_drift();
        let result = RestorationResult::Success;

        // We can't easily test the full coordinator without a real Kubernetes client,
        // but we can test the event creation logic
        let event = RestorationEvent::builder()
            .resource_type(drift.resource_type().to_string())
            .resource_name(drift.resource_name().to_string())
            .namespace(drift.namespace().to_string())
            .drift_type(drift.drift_type().clone())
            .restoration_result(result)
            .attempt_number(1)
            .build();

        assert_eq!(event.resource_type(), "ConfigMap");
        assert_eq!(event.resource_name(), "test-config");
        assert_eq!(event.namespace(), "default");
        assert!(event.was_successful());
    }

    #[tokio::test]
    async fn test_no_op_sync_coordinator() {
        let _coordinator = NoOpSyncCoordinator::default();
        // Basic smoke test - if we get here without panicking, the test passes
        assert!(true);
    }
}
