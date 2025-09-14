use getset::{Getters, MutGetters};
use k8s_openapi::chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use typed_builder::TypedBuilder;

/// Types of drift that can be detected in resources
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftType {
    /// Resource has been modified from its expected state
    Modified,
    /// Resource has been deleted
    Deleted,
    /// An unexpected resource was created
    UnexpectedCreation,
}

impl std::fmt::Display for DriftType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftType::Modified => write!(f, "Modified"),
            DriftType::Deleted => write!(f, "Deleted"),
            DriftType::UnexpectedCreation => write!(f, "UnexpectedCreation"),
        }
    }
}

/// Represents detected configuration drift in a resource
#[derive(Debug, Clone, Serialize, Deserialize, Getters, MutGetters, TypedBuilder)]
pub struct ResourceDrift<T> {
    /// Name of the resource that drifted
    #[getset(get = "pub")]
    #[builder(setter(into))]
    resource_name: String,

    /// Namespace of the resource
    #[getset(get = "pub")]
    #[builder(setter(into))]
    namespace: String,

    /// The actual resource state (if it exists)
    #[getset(get = "pub", get_mut = "pub")]
    actual_resource: Option<T>,

    /// The expected resource state
    #[getset(get = "pub", get_mut = "pub")]
    expected_resource: T,

    /// Type of drift detected
    #[getset(get = "pub")]
    drift_type: DriftType,

    /// When the drift was detected
    #[getset(get = "pub")]
    #[builder(default_code = "Utc::now()")]
    detected_at: DateTime<Utc>,

    /// Resource type for logging and identification
    #[getset(get = "pub")]
    #[builder(setter(into))]
    resource_type: String,
}

impl<T> ResourceDrift<T> {
    /// Get a human-readable description of the drift
    pub fn description(&self) -> String {
        match self.drift_type() {
            DriftType::Modified => {
                format!(
                    "{} {}/{} in namespace {} was modified",
                    self.resource_type(),
                    self.resource_name(),
                    self.resource_name(),
                    self.namespace()
                )
            }
            DriftType::Deleted => {
                format!(
                    "{} {}/{} in namespace {} was deleted",
                    self.resource_type(),
                    self.resource_name(),
                    self.resource_name(),
                    self.namespace()
                )
            }
            DriftType::UnexpectedCreation => {
                format!(
                    "Unexpected {} {}/{} was created in namespace {}",
                    self.resource_type(),
                    self.resource_name(),
                    self.resource_name(),
                    self.namespace()
                )
            }
        }
    }

    /// Check if this drift requires restoration
    pub fn requires_restoration(&self) -> bool {
        matches!(self.drift_type(), DriftType::Modified | DriftType::Deleted)
    }
}

/// Result of a restoration operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestorationResult {
    /// Restoration was successful
    Success,
    /// Restoration failed with an error message
    Failed(String),
    /// Restoration was skipped with a reason
    Skipped(String),
}

impl std::fmt::Display for RestorationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestorationResult::Success => write!(f, "Success"),
            RestorationResult::Failed(msg) => write!(f, "Failed: {msg}"),
            RestorationResult::Skipped(reason) => write!(f, "Skipped: {reason}"),
        }
    }
}

/// Audit trail for restoration operations
#[derive(Debug, Clone, Serialize, Deserialize, Getters, MutGetters, TypedBuilder)]
pub struct RestorationEvent {
    /// Type of resource that was restored
    #[getset(get = "pub")]
    #[builder(setter(into))]
    resource_type: String,

    /// Name of the resource
    #[getset(get = "pub")]
    #[builder(setter(into))]
    resource_name: String,

    /// Namespace of the resource
    #[getset(get = "pub")]
    #[builder(setter(into))]
    namespace: String,

    /// Type of drift that triggered the restoration
    #[getset(get = "pub")]
    drift_type: DriftType,

    /// Result of the restoration operation
    #[getset(get = "pub", get_mut = "pub")]
    restoration_result: RestorationResult,

    /// When the restoration was attempted
    #[getset(get = "pub")]
    #[builder(default_code = "Utc::now()")]
    timestamp: DateTime<Utc>,

    /// Which attempt this was (1-based)
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default = 1)]
    attempt_number: u32,

    /// Duration of the restoration operation
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default)]
    duration: Option<Duration>,
}

impl RestorationEvent {
    /// Check if this restoration was successful
    pub fn was_successful(&self) -> bool {
        matches!(self.restoration_result(), RestorationResult::Success)
    }

    /// Check if this restoration failed
    pub fn was_failed(&self) -> bool {
        matches!(self.restoration_result(), RestorationResult::Failed(_))
    }

    /// Check if this restoration was skipped
    pub fn was_skipped(&self) -> bool {
        matches!(self.restoration_result(), RestorationResult::Skipped(_))
    }

    /// Get a human-readable description of the event
    pub fn description(&self) -> String {
        format!(
            "Restoration attempt {} for {} {}/{} in namespace {} (drift: {}): {}",
            self.attempt_number(),
            self.resource_type(),
            self.resource_name(),
            self.resource_name(),
            self.namespace(),
            self.drift_type(),
            self.restoration_result()
        )
    }
}

/// Retry policy configuration for restoration operations
#[derive(Debug, Clone, Serialize, Deserialize, Getters, MutGetters, TypedBuilder)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default = 3)]
    max_attempts: u32,

    /// Base delay between retries
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default_code = "Duration::from_secs(1)")]
    base_delay: Duration,

    /// Maximum delay between retries
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default_code = "Duration::from_secs(30)")]
    max_delay: Duration,

    /// Multiplier for exponential backoff
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default = 2.0)]
    backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl RetryPolicy {
    /// Calculate the delay for a given attempt number (1-based)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 1 {
            return Duration::from_secs(0); // No delay for first attempt
        }

        let delay_secs =
            self.base_delay().as_secs_f64() * self.backoff_multiplier().powi((attempt - 2) as i32);

        let delay = Duration::from_secs_f64(delay_secs.min(self.max_delay().as_secs_f64()));

        std::cmp::min(delay, *self.max_delay())
    }

    /// Check if we should retry for the given attempt number
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt <= *self.max_attempts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_drift_type_display() {
        assert_eq!(DriftType::Modified.to_string(), "Modified");
        assert_eq!(DriftType::Deleted.to_string(), "Deleted");
        assert_eq!(
            DriftType::UnexpectedCreation.to_string(),
            "UnexpectedCreation"
        );
    }

    #[test]
    fn test_drift_type_serialization() {
        let drift_type = DriftType::Modified;
        let json = serde_json::to_string(&drift_type).unwrap();
        let deserialized: DriftType = serde_json::from_str(&json).unwrap();
        assert_eq!(drift_type, deserialized);
    }

    #[test]
    fn test_resource_drift_creation() {
        let drift = ResourceDrift::<String>::builder()
            .resource_name("test-config")
            .namespace("default")
            .actual_resource(Some("actual".to_string()))
            .expected_resource("expected".to_string())
            .drift_type(DriftType::Modified)
            .resource_type("ConfigMap")
            .build();

        assert_eq!(drift.resource_name(), "test-config");
        assert_eq!(drift.namespace(), "default");
        assert_eq!(drift.drift_type(), &DriftType::Modified);
        assert!(drift.requires_restoration());
    }

    #[test]
    fn test_resource_drift_description() {
        let drift = ResourceDrift::<String>::builder()
            .resource_name("test-config")
            .namespace("default")
            .actual_resource(None)
            .expected_resource("expected".to_string())
            .drift_type(DriftType::Deleted)
            .resource_type("ConfigMap")
            .build();

        let description = drift.description();
        assert!(description.contains("ConfigMap"));
        assert!(description.contains("test-config"));
        assert!(description.contains("default"));
        assert!(description.contains("deleted"));
    }

    #[test]
    fn test_restoration_result_display() {
        assert_eq!(RestorationResult::Success.to_string(), "Success");
        assert_eq!(
            RestorationResult::Failed("error".to_string()).to_string(),
            "Failed: error"
        );
        assert_eq!(
            RestorationResult::Skipped("reason".to_string()).to_string(),
            "Skipped: reason"
        );
    }

    #[test]
    fn test_restoration_event_creation() {
        let event = RestorationEvent::builder()
            .resource_type("ConfigMap")
            .resource_name("test-config")
            .namespace("default")
            .drift_type(DriftType::Modified)
            .restoration_result(RestorationResult::Success)
            .build();

        assert_eq!(event.resource_type(), "ConfigMap");
        assert_eq!(event.attempt_number(), &1);
        assert!(event.was_successful());
        assert!(!event.was_failed());
        assert!(!event.was_skipped());
    }

    #[test]
    fn test_restoration_event_description() {
        let event = RestorationEvent::builder()
            .resource_type("ConfigMap")
            .resource_name("test-config")
            .namespace("default")
            .drift_type(DriftType::Modified)
            .restoration_result(RestorationResult::Success)
            .attempt_number(2)
            .build();

        let description = event.description();
        assert!(description.contains("attempt 2"));
        assert!(description.contains("ConfigMap"));
        assert!(description.contains("test-config"));
        assert!(description.contains("Success"));
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts(), &3);
        assert_eq!(policy.base_delay(), &Duration::from_secs(1));
        assert_eq!(policy.max_delay(), &Duration::from_secs(30));
        assert!((policy.backoff_multiplier() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_retry_policy_delay_calculation() {
        let policy = RetryPolicy::default();

        // First attempt has no delay
        assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(0));

        // Second attempt has base delay
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(1));

        // Third attempt has doubled delay
        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(2));

        // Fourth attempt has quadrupled delay
        assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(4));
    }

    #[test]
    fn test_retry_policy_max_delay() {
        let policy = RetryPolicy::builder()
            .max_delay(Duration::from_secs(5))
            .build();

        // Should cap at max_delay
        assert!(policy.delay_for_attempt(10) <= Duration::from_secs(5));
    }

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::builder().max_attempts(3).build();

        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    #[test]
    fn test_data_model_serialization() {
        let drift = ResourceDrift::<String>::builder()
            .resource_name("test")
            .namespace("default")
            .actual_resource(Some("actual".to_string()))
            .expected_resource("expected".to_string())
            .drift_type(DriftType::Modified)
            .resource_type("ConfigMap")
            .build();

        let json = serde_json::to_string(&drift).unwrap();
        let deserialized: ResourceDrift<String> = serde_json::from_str(&json).unwrap();

        assert_eq!(drift.resource_name(), deserialized.resource_name());
        assert_eq!(drift.drift_type(), deserialized.drift_type());
    }
}
