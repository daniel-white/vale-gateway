use crate::gateways::{GatewayConfigurationMergerService, GatewaySyncController, collectors};
use crate::ipc::instances::InstanceRole;
use crate::kubernetes::{KubeClientCell, objects::ObjectRef};
use crate::options::Options;
use std::sync::Arc;
use tracing::info;
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;

/// Main orchestrator for gateway configuration and resource management
pub struct GatewayOrchestrator;

impl GatewayOrchestrator {
    /// Spawn all gateway-related controllers and services
    pub fn spawn_all(
        task_builder: &TaskBuilder,
        options: Arc<Options>,
        kube_client_rx: &Receiver<KubeClientCell>,
        ipc_config: Arc<vg_core::ipc::IpcConfiguration>,
        ipc_services: Arc<crate::ipc::IpcServices>,
    ) {
        info!("Starting gateway orchestrator");

        // Create a simple instance role (always primary for now)
        let (instance_role_tx, instance_role_rx) = signal("instance_role");
        let default_object_ref = ObjectRef::builder()
            .kind("Pod")
            .name("vale-gateway-controller")
            .namespace(Some("default".to_string()))
            .build();
        task_builder
            .new_task("set_instance_role")
            .spawn(async move {
                instance_role_tx
                    .set(InstanceRole::Primary(default_object_ref))
                    .await;
            });

        // Start the gateway collectors
        let gateways_rx = collectors::gateways(task_builder, options.clone(), kube_client_rx);

        // Start the configuration merger
        let configurations_rx =
            GatewayConfigurationMergerService::merged_configurations(task_builder, &gateways_rx);

        // Feed configurations to IPC services
        Self::feed_configurations_to_ipc(task_builder, configurations_rx.clone(), ipc_services);

        // Start the resource synchronization controllers
        let sync_controller = GatewaySyncController::builder()
            .options(options.clone())
            .build();
        sync_controller.spawn_all(
            task_builder,
            kube_client_rx.clone(),
            instance_role_rx,
            configurations_rx,
            ipc_config,
        );

        info!("Gateway orchestrator started successfully");
    }

    /// Feed gateway configurations to IPC services so gateway instances can retrieve them
    fn feed_configurations_to_ipc(
        task_builder: &TaskBuilder,
        configurations_rx: Receiver<crate::gateways::resources::GatewayResourceConfigurations>,
        ipc_services: Arc<crate::ipc::IpcServices>,
    ) {
        use crate::kubernetes::objects::ObjectRef;
        use gateway_api::apis::standard::gateways::Gateway;
        use vg_core::{ReadyState, await_ready, continue_on};

        task_builder
            .new_task("feed_configurations_to_ipc")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready(configurations) = await_ready!(configurations_rx) {
                        info!(
                            "Feeding {} gateway configurations to IPC services",
                            configurations.len()
                        );

                        for (name, config) in configurations.iter() {
                            let gateway_ref = ObjectRef::of_kind::<Gateway>()
                                .name(name)
                                .namespace(Some(config.namespace().clone()))
                                .build();

                            // Convert the gateway configuration to the API format
                            let gateway_config = config.gateway_config().clone();

                            // Feed the configuration to IPC services
                            if let Err(err) = ipc_services.try_insert_gateway_configuration(
                                gateway_ref.clone(),
                                gateway_config,
                            ) {
                                tracing::error!(
                                    "Failed to insert gateway configuration for {}: {}",
                                    gateway_ref,
                                    err
                                );
                            } else {
                                tracing::debug!(
                                    "Successfully inserted gateway configuration for {}",
                                    gateway_ref
                                );
                            }
                        }
                    }

                    continue_on!(configurations_rx.changed());
                }
            });
    }
}

/// Utility functions for gateway management
pub struct GatewayUtils;

impl GatewayUtils {
    /// Validate gateway configuration
    pub fn validate_configuration(
        _gateway_name: &str,
        _gateway_namespace: &str,
    ) -> Result<(), String> {
        // Add validation logic here
        Ok(())
    }

    /// Generate resource names for a gateway
    pub fn resource_names(gateway_name: &str) -> GatewayResourceNames {
        GatewayResourceNames {
            deployment: gateway_name.to_string(),
            service: gateway_name.to_string(),
            config_map: format!("{}-config", gateway_name),
        }
    }
}

/// Standard resource names for a gateway
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayResourceNames {
    pub deployment: String,
    pub service: String,
    pub config_map: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_names_generation() {
        let names = GatewayUtils::resource_names("my-gateway");

        assert_eq!(names.deployment, "my-gateway");
        assert_eq!(names.service, "my-gateway");
        assert_eq!(names.config_map, "my-gateway-config");
    }

    #[test]
    fn test_configuration_validation() {
        let result = GatewayUtils::validate_configuration("test-gateway", "default");
        assert!(result.is_ok());
    }
}
