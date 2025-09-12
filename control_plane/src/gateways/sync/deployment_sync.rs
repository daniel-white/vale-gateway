use crate::gateways::resources::{GatewayResourceConfiguration, GatewayResourceConfigurations};
use crate::ipc::instances::InstanceRole;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::apps::v1::Deployment;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};
use typed_builder::TypedBuilder;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_after, ReadyState};

const TEMPLATE: &str = include_str!("./templates/gateway_deployment.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into))]
    configmap_name: String,
    #[builder(setter(into), default = "ghcr.io/daniel-white/vale-gateway".to_string())]
    image_repository: String,
    #[builder(setter(into), default = "latest".to_string())]
    image_tag: String,
    #[builder(setter(into), default = "IfNotPresent".to_string())]
    image_pull_policy: String,
    #[builder(default = 1)]
    replicas: i32,
}

/// Synchronizer for Kubernetes Deployments
#[derive(Debug, Clone)]
pub struct DeploymentSynchronizer;

impl DeploymentSynchronizer {
    pub fn new() -> Self {
        Self
    }

    /// Get the default container image for vale-gateway
    fn default_gateway_image() -> String {
        std::env::var("VALE_GATEWAY_IMAGE")
            .unwrap_or_else(|_| "ghcr.io/daniel-white/vale-gateway:latest".to_string())
    }

    /// Sync all deployments from gateway configurations using templates
    pub fn sync_all_deployments(
        &self,
        task_builder: &TaskBuilder,
        options: Arc<Options>,
        kube_client_rx: Receiver<KubeClientCell>,
        instance_role_rx: Receiver<InstanceRole>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
    ) {
        let (tx, current_refs_rx) = sync_objects!(
            options,
            task_builder,
            Deployment,
            kube_client_rx,
            instance_role_rx,
            TemplateValues,
            TEMPLATE
        );

        DeploymentSynchronizer::generate_deployments(
            task_builder,
            tx,
            current_refs_rx,
            configurations_rx,
        );
    }

    fn generate_deployments(
        task_builder: &TaskBuilder,
        sync_tx: UnboundedSender<SyncObjectAction<TemplateValues, Deployment>>,
        current_refs_rx: Receiver<HashSet<ObjectRef>>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
    ) {
        task_builder
            .new_task("generate_gateway_deployments")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready((configurations, current_deployment_refs)) =
                        await_ready!(configurations_rx, current_refs_rx)
                    {
                        info!("Reconciling Gateway Deployments");

                        let desired_deployments =
                            DeploymentSynchronizer::expand_configurations(&configurations);
                        let desired_deployment_refs: HashSet<_> = desired_deployments
                            .iter()
                            .map(|state| state.deployment_ref.clone())
                            .collect();

                        // Delete removed deployments
                        let deleted_refs =
                            current_deployment_refs.difference(&desired_deployment_refs);
                        for deleted_ref in deleted_refs {
                            if let Err(err) =
                                sync_tx.send(SyncObjectAction::Delete(deleted_ref.clone()))
                            {
                                error!("Failed to send delete action: {}", err);
                            }
                        }

                        // Create/update desired deployments
                        for deployment_state in desired_deployments {
                            if let Some(template_values) = deployment_state.template_values {
                                if let Err(err) = sync_tx.send(SyncObjectAction::Upsert(
                                    deployment_state.deployment_ref,
                                    deployment_state.gateway_ref,
                                    template_values,
                                    None,
                                )) {
                                    error!("Failed to send upsert action: {}", err);
                                }
                            }
                        }
                    }

                    continue_after!(
                        std::time::Duration::from_secs(30),
                        configurations_rx.changed(),
                        current_refs_rx.changed()
                    );
                }
            });
    }

    fn expand_configurations(
        configurations: &GatewayResourceConfigurations,
    ) -> Vec<DeploymentState> {
        configurations
            .iter()
            .map(|(name, config)| {
                let gateway_ref = ObjectRef::builder()
                    .kind("Gateway")
                    .name(name)
                    .namespace(config.namespace().clone())
                    .build();

                let deployment_ref = ObjectRef::builder()
                    .kind("Deployment")
                    .name(name)
                    .namespace(config.namespace().clone())
                    .build();

                let template_values =
                    DeploymentSynchronizer::generate_template_values(name, config);

                DeploymentState {
                    gateway_ref,
                    deployment_ref,
                    template_values,
                }
            })
            .collect()
    }

    fn generate_template_values(
        gateway_name: &str,
        config: &GatewayResourceConfiguration,
    ) -> Option<TemplateValues> {
        let image_parts = Self::default_gateway_image();
        let (image_repository, image_tag) = if let Some((repo, tag)) = image_parts.rsplit_once(':')
        {
            (repo.to_string(), tag.to_string())
        } else {
            (image_parts, "latest".to_string())
        };

        Some(
            TemplateValues::builder()
                .gateway_name(gateway_name)
                .configmap_name(&config.config_map_name())
                .image_repository(image_repository)
                .image_tag(image_tag)
                .replicas(1)
                .build(),
        )
    }
}

#[derive(Debug)]
struct DeploymentState {
    gateway_ref: ObjectRef,
    deployment_ref: ObjectRef,
    template_values: Option<TemplateValues>,
}

impl Default for DeploymentSynchronizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deployment_synchronizer_creation() {
        let _sync = DeploymentSynchronizer::new();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_deployment_synchronizer_default() {
        let _sync = DeploymentSynchronizer::default();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_default_gateway_image() {
        let image = DeploymentSynchronizer::default_gateway_image();
        assert!(image.contains("vale-gateway"));
    }

    #[test]
    fn test_template_values_creation() {
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .configmap_name("test-gateway-config")
            .build();

        assert_eq!(template_values.gateway_name, "test-gateway");
        assert_eq!(template_values.configmap_name, "test-gateway-config");
        assert_eq!(template_values.replicas, 1);
    }
}
