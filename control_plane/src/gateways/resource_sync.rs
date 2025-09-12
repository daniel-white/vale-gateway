use crate::gateways::resources::GatewayResourceConfigurations;
use crate::kubernetes::objects::ObjectRef;
use crate::kubernetes::KubeClientCell;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use kube::api::{Patch, PatchParams};
use kube::{Api, Client};
use std::collections::HashMap;
use tracing::{debug, error, info};
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

/// Service responsible for synchronizing gateway resources to Kubernetes
pub struct GatewayResourceSyncService;

impl GatewayResourceSyncService {
    /// Sync all gateway resources (deployments, services, configmaps) to Kubernetes
    pub fn sync_all_resources(
        task_builder: &TaskBuilder,
        kube_client_rx: &Receiver<KubeClientCell>,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) {
        Self::sync_deployments(task_builder, kube_client_rx, configurations_rx);
        Self::sync_services(task_builder, kube_client_rx, configurations_rx);
        Self::sync_config_maps(task_builder, kube_client_rx, configurations_rx);
    }

    /// Sync gateway deployments to Kubernetes
    pub fn sync_deployments(
        task_builder: &TaskBuilder,
        kube_client_rx: &Receiver<KubeClientCell>,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) {
        let kube_client_rx = kube_client_rx.clone();
        let configurations_rx = configurations_rx.clone();

        task_builder
            .new_task("sync_gateway_deployments")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready((kube_client, configurations)) =
                        await_ready!(kube_client_rx, configurations_rx)
                    {
                        {
                            let client: Client = kube_client.clone().into();
                            info!("Syncing {} gateway deployments", configurations.len());

                            for (name, config) in configurations.iter() {
                                if let Err(e) = Self::sync_single_deployment(
                                    client.clone(),
                                    name,
                                    config.deployment(),
                                    config.namespace(),
                                )
                                .await
                                {
                                    error!("Failed to sync deployment {}: {}", name, e);
                                } else {
                                    debug!("Successfully synced deployment {}", name);
                                }
                            }
                        }
                    }

                    continue_on!(kube_client_rx.changed(), configurations_rx.changed());
                }
            });
    }

    /// Sync gateway services to Kubernetes
    pub fn sync_services(
        task_builder: &TaskBuilder,
        kube_client_rx: &Receiver<KubeClientCell>,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) {
        let kube_client_rx = kube_client_rx.clone();
        let configurations_rx = configurations_rx.clone();

        task_builder
            .new_task("sync_gateway_services")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready((kube_client, configurations)) =
                        await_ready!(kube_client_rx, configurations_rx)
                    {
                        {
                            let client: Client = kube_client.clone().into();
                            info!("Syncing {} gateway services", configurations.len());

                            for (name, config) in configurations.iter() {
                                if let Err(e) = Self::sync_single_service(
                                    client.clone(),
                                    name,
                                    config.service(),
                                    config.namespace(),
                                )
                                .await
                                {
                                    error!("Failed to sync service {}: {}", name, e);
                                } else {
                                    debug!("Successfully synced service {}", name);
                                }
                            }
                        }
                    }

                    continue_on!(kube_client_rx.changed(), configurations_rx.changed());
                }
            });
    }

    /// Sync gateway config maps to Kubernetes
    pub fn sync_config_maps(
        task_builder: &TaskBuilder,
        kube_client_rx: &Receiver<KubeClientCell>,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) {
        let kube_client_rx = kube_client_rx.clone();
        let configurations_rx = configurations_rx.clone();

        task_builder
            .new_task("sync_gateway_config_maps")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready((kube_client, configurations)) =
                        await_ready!(kube_client_rx, configurations_rx)
                    {
                        {
                            let client: Client = kube_client.clone().into();
                            info!("Syncing {} gateway config maps", configurations.len());

                            for (name, config) in configurations.iter() {
                                let config_map_name = config.config_map_name();
                                if let Err(e) = Self::sync_single_config_map(
                                    client.clone(),
                                    &config_map_name,
                                    config.config_map(),
                                    config.namespace(),
                                )
                                .await
                                {
                                    error!("Failed to sync config map {}: {}", config_map_name, e);
                                } else {
                                    debug!("Successfully synced config map {}", config_map_name);
                                }
                            }
                        }
                    }

                    continue_on!(kube_client_rx.changed(), configurations_rx.changed());
                }
            });
    }

    async fn sync_single_deployment(
        client: Client,
        name: &str,
        deployment: &Deployment,
        namespace: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api: Api<Deployment> = Api::namespaced(client, namespace);

        let patch_params = PatchParams::apply("vale-gateway-controller");
        let patch = Patch::Apply(deployment);

        match api.patch(name, &patch_params, &patch).await {
            Ok(_) => {
                debug!("Applied deployment patch for {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to apply deployment patch for {}: {}", name, e);
                Err(Box::new(e))
            }
        }
    }

    async fn sync_single_service(
        client: Client,
        name: &str,
        service: &Service,
        namespace: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api: Api<Service> = Api::namespaced(client, namespace);

        let patch_params = PatchParams::apply("vale-gateway-controller");
        let patch = Patch::Apply(service);

        match api.patch(name, &patch_params, &patch).await {
            Ok(_) => {
                debug!("Applied service patch for {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to apply service patch for {}: {}", name, e);
                Err(Box::new(e))
            }
        }
    }

    async fn sync_single_config_map(
        client: Client,
        name: &str,
        config_map: &ConfigMap,
        namespace: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api: Api<ConfigMap> = Api::namespaced(client, namespace);

        let patch_params = PatchParams::apply("vale-gateway-controller");
        let patch = Patch::Apply(config_map);

        match api.patch(name, &patch_params, &patch).await {
            Ok(_) => {
                debug!("Applied config map patch for {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to apply config map patch for {}: {}", name, e);
                Err(Box::new(e))
            }
        }
    }
}

/// Service for managing resource lifecycle (creation, updates, deletion)
pub struct GatewayResourceLifecycleService;

impl GatewayResourceLifecycleService {
    /// Handle resource actions (create, update, delete) based on configuration changes
    pub fn handle_resource_actions(
        _task_builder: &TaskBuilder,
        _kube_client_rx: &Receiver<KubeClientCell>,
        _resource_actions_rx: &Receiver<HashMap<ObjectRef, String>>,
    ) {
        // Placeholder for future implementation
        info!("Resource lifecycle service initialized");
    }
}
