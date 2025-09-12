use crate::gateways::resources::GatewayResourceConfigurations;
use crate::ipc::instances::InstanceRole;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use getset::Getters;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;
use typed_builder::TypedBuilder;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Template error: {0}")]
    Template(String),
}

/// Main sync controller that coordinates all resource synchronization
#[derive(Debug, Getters, TypedBuilder)]
pub struct GatewaySyncController {
    #[getset(get = "pub")]
    options: Arc<Options>,
}

impl GatewaySyncController {
    /// Spawn all sync controllers
    pub fn spawn_all(
        &self,
        task_builder: &TaskBuilder,
        kube_client_rx: Receiver<KubeClientCell>,
        instance_role_rx: Receiver<InstanceRole>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
        ipc_config: Arc<vg_core::ipc::IpcConfiguration>,
    ) {
        info!("Starting gateway sync controllers");

        // Spawn deployment sync controller
        let deployment_sync = super::DeploymentSynchronizer::new();
        deployment_sync.sync_all_deployments(
            task_builder,
            self.options.clone(),
            kube_client_rx.clone(),
            instance_role_rx.clone(),
            configurations_rx.clone(),
        );

        // Spawn service sync controller
        let service_sync = super::ServiceSynchronizer::new();
        service_sync.sync_all_services(
            task_builder,
            self.options.clone(),
            kube_client_rx.clone(),
            instance_role_rx.clone(),
            configurations_rx.clone(),
        );

        // Spawn configmap sync controller
        let configmap_sync = super::ConfigMapSynchronizer::new();
        configmap_sync.sync_all_configmaps(
            task_builder,
            self.options.clone(),
            kube_client_rx,
            instance_role_rx,
            configurations_rx,
            ipc_config,
        );

        info!("Gateway sync controllers started");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;

    #[test]
    fn test_gateway_sync_controller_creation() {
        let options = Arc::new(Options::default());
        let _controller = GatewaySyncController::builder().options(options).build();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_sync_error_display() {
        let error = SyncError::Configuration("test error".to_string());
        assert!(error.to_string().contains("test error"));

        let error = SyncError::Template("template error".to_string());
        assert!(error.to_string().contains("template error"));
    }
}
