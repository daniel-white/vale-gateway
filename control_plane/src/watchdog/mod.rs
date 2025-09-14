pub mod configuration;
pub mod data_models;
pub mod drift_detector;
pub mod error;
pub mod restoration_coordinator;
pub mod service;

pub use configuration::WatchdogConfiguration;
pub use data_models::{DriftType, ResourceDrift, RestorationEvent, RestorationResult, RetryPolicy};
pub use drift_detector::{detect_unexpected_creation, DefaultDriftDetector, DriftDetector};
pub use error::WatchdogError;
pub use restoration_coordinator::{NoOpSyncCoordinator, RestorationCoordinator, SyncCoordinator};
pub use service::ResourceWatchdogService;
