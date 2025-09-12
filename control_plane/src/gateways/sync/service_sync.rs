use crate::gateways::resources::{GatewayResourceConfiguration, GatewayResourceConfigurations};
use crate::ipc::instances::InstanceRole;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::Service;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info};
use typed_builder::TypedBuilder;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_after, ReadyState};

const TEMPLATE: &str = include_str!("./templates/gateway_service.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into), default = "ClusterIP".to_string())]
    service_type: String,
}

/// Synchronizer for Kubernetes Services
#[derive(Debug, Clone)]
pub struct ServiceSynchronizer;

impl ServiceSynchronizer {
    pub fn new() -> Self {
        Self
    }

    /// Sync all services from gateway configurations using templates
    pub fn sync_all_services(
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
            Service,
            kube_client_rx,
            instance_role_rx,
            TemplateValues,
            TEMPLATE
        );

        ServiceSynchronizer::generate_services(
            task_builder,
            tx,
            current_refs_rx,
            configurations_rx,
        );
    }

    fn generate_services(
        task_builder: &TaskBuilder,
        sync_tx: UnboundedSender<SyncObjectAction<TemplateValues, Service>>,
        current_refs_rx: Receiver<HashSet<ObjectRef>>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
    ) {
        task_builder
            .new_task("generate_gateway_services")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready((configurations, current_service_refs)) =
                        await_ready!(configurations_rx, current_refs_rx)
                    {
                        info!("Reconciling Gateway Services");

                        let desired_services =
                            ServiceSynchronizer::expand_configurations(&configurations);
                        let desired_service_refs: HashSet<_> = desired_services
                            .iter()
                            .map(|state| state.service_ref.clone())
                            .collect();

                        // Delete removed services
                        let deleted_refs = current_service_refs.difference(&desired_service_refs);
                        for deleted_ref in deleted_refs {
                            if let Err(err) =
                                sync_tx.send(SyncObjectAction::Delete(deleted_ref.clone()))
                            {
                                error!("Failed to send delete action: {}", err);
                            }
                        }

                        // Create/update desired services
                        for service_state in desired_services {
                            if let Some(template_values) = service_state.template_values {
                                if let Err(err) = sync_tx.send(SyncObjectAction::Upsert(
                                    service_state.service_ref,
                                    service_state.gateway_ref,
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

    fn expand_configurations(configurations: &GatewayResourceConfigurations) -> Vec<ServiceState> {
        configurations
            .iter()
            .map(|(name, config)| {
                let gateway_ref = ObjectRef::builder()
                    .kind("Gateway")
                    .name(name)
                    .namespace(config.namespace().clone())
                    .build();

                let service_ref = ObjectRef::builder()
                    .kind("Service")
                    .name(name)
                    .namespace(config.namespace().clone())
                    .build();

                let template_values = ServiceSynchronizer::generate_template_values(name, config);

                ServiceState {
                    gateway_ref,
                    service_ref,
                    template_values,
                }
            })
            .collect()
    }

    fn generate_template_values(
        gateway_name: &str,
        _config: &GatewayResourceConfiguration,
    ) -> Option<TemplateValues> {
        Some(
            TemplateValues::builder()
                .gateway_name(gateway_name)
                .service_type("ClusterIP")
                .build(),
        )
    }
}

#[derive(Debug)]
struct ServiceState {
    gateway_ref: ObjectRef,
    service_ref: ObjectRef,
    template_values: Option<TemplateValues>,
}

impl Default for ServiceSynchronizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_synchronizer_creation() {
        let _sync = ServiceSynchronizer::new();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_service_synchronizer_default() {
        let _sync = ServiceSynchronizer::default();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_template_values_creation() {
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .build();

        assert_eq!(template_values.gateway_name, "test-gateway");
        assert_eq!(template_values.service_type, "ClusterIP");
    }
}
