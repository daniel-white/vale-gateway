use thiserror::Error;

/// Errors that can occur in the watchdog service
#[derive(Debug, Error)]
pub enum WatchdogError {
    #[error("Kubernetes API error: {0}")]
    KubernetesApi(#[from] kube::Error),

    #[error("Resource restoration failed: {0}")]
    RestorationFailed(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Ownership validation failed: {0}")]
    OwnershipValidation(String),

    #[error("Sync coordination error: {0}")]
    SyncCoordination(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Task execution error: {0}")]
    TaskExecution(String),

    #[error("Resource not found: {resource_type}/{name} in namespace {namespace}")]
    ResourceNotFound {
        resource_type: String,
        name: String,
        namespace: String,
    },
}

impl WatchdogError {
    /// Create a new configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration(message.into())
    }

    /// Create a new restoration failed error
    pub fn restoration_failed<S: Into<String>>(message: S) -> Self {
        Self::RestorationFailed(message.into())
    }

    /// Create a new ownership validation error
    pub fn ownership_validation<S: Into<String>>(message: S) -> Self {
        Self::OwnershipValidation(message.into())
    }

    /// Create a new sync coordination error
    pub fn sync_coordination<S: Into<String>>(message: S) -> Self {
        Self::SyncCoordination(message.into())
    }

    /// Create a new task execution error
    pub fn task_execution<S: Into<String>>(message: S) -> Self {
        Self::TaskExecution(message.into())
    }

    /// Create a new resource not found error
    pub fn resource_not_found<S: Into<String>>(resource_type: S, name: S, namespace: S) -> Self {
        Self::ResourceNotFound {
            resource_type: resource_type.into(),
            name: name.into(),
            namespace: namespace.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = WatchdogError::configuration("test config error");
        assert!(error.to_string().contains("test config error"));

        let error = WatchdogError::restoration_failed("restoration failed");
        assert!(error.to_string().contains("restoration failed"));

        let error = WatchdogError::resource_not_found("ConfigMap", "test-config", "default");
        assert!(error.to_string().contains("ConfigMap/test-config"));
        assert!(error.to_string().contains("default"));
    }

    #[test]
    fn test_error_from_kube_error() {
        let kube_error = kube::Error::Api(kube::error::ErrorResponse {
            status: "Failure".to_string(),
            message: "test error".to_string(),
            reason: "BadRequest".to_string(),
            code: 400,
        });

        let watchdog_error: WatchdogError = kube_error.into();
        assert!(matches!(watchdog_error, WatchdogError::KubernetesApi(_)));
    }

    #[test]
    fn test_error_from_serde_error() {
        let json_str = r#"{"invalid": json}"#;
        let serde_error: serde_json::Error =
            serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();

        let watchdog_error: WatchdogError = serde_error.into();
        assert!(matches!(watchdog_error, WatchdogError::Serialization(_)));
    }
}
