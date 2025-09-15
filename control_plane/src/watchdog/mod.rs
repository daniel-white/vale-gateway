pub mod configmap_restoration;
pub mod configmap_watcher;
pub mod configuration;
pub mod data_models;
pub mod drift_detector;
pub mod error;
pub mod resource_watcher;
pub mod restoration_coordinator;
pub mod service;

#[cfg(test)]
pub mod tests;

pub use configmap_restoration::create_configmap_restoration_coordinator;
pub use configmap_watcher::ConfigMapWatcher;
pub use configuration::WatchdogConfiguration;
pub use data_models::{DriftType, ResourceDrift, RestorationEvent, RestorationResult, RetryPolicy};
pub use drift_detector::DefaultDriftDetector;
pub use error::WatchdogError;
pub use resource_watcher::{ResourceWatcher, WatchEvent, WatcherConfig};
