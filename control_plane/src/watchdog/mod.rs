pub mod configuration;
pub mod data_models;
pub mod drift_detector;
pub mod error;
pub mod resource_watcher;
pub mod restoration_coordinator;
pub mod service;

pub use configuration::WatchdogConfiguration;
pub use data_models::{DriftType, ResourceDrift, RestorationEvent, RestorationResult, RetryPolicy};
pub use drift_detector::DriftDetector;
pub use error::WatchdogError;
pub use resource_watcher::{ResourceWatcher, ResourceWatcherBuilder, WatchEvent, WatcherConfig};
pub use restoration_coordinator::SyncCoordinator;
