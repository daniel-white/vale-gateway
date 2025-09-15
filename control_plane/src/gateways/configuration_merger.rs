use crate::gateways::collectors::Gateways;
use crate::gateways::resources::{
    GatewayResourceConfiguration, GatewayResourceConfigurations, GatewayResourceMetadata,
};
use crate::kubernetes::objects::ObjectRef;
use gateway_api::apis::standard::gateways::Gateway;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use vg_api::v1alpha1::{GatewayClassParameters, GatewayConfiguration, GatewayParameters};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

// Temporary simplified merged configuration until we implement the full merger
#[derive(Debug, Clone)]
pub struct MergedGatewayConfiguration {
    deployment: Deployment,
    service: Service,
    config_map: ConfigMap,
    image_repository: String,
    image_tag: String,
    gateway_config: GatewayConfiguration,
    open_telemetry: Option<vg_api::v1alpha1::GatewayInstrumentationOpenTelemetry>,
}

impl MergedGatewayConfiguration {
    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }

    pub fn service(&self) -> &Service {
        &self.service
    }

    pub fn config_map(&self) -> &ConfigMap {
        &self.config_map
    }

    pub fn image_repository(&self) -> &str {
        &self.image_repository
    }

    pub fn image_tag(&self) -> &str {
        &self.image_tag
    }

    pub fn gateway_config(&self) -> &GatewayConfiguration {
        &self.gateway_config
    }

    pub fn open_telemetry(&self) -> &Option<vg_api::v1alpha1::GatewayInstrumentationOpenTelemetry> {
        &self.open_telemetry
    }
}

// Temporary simplified configuration merger
pub struct ConfigurationMerger;

impl ConfigurationMerger {
    pub fn merge(
        _gateway_name: &str,
        _gateway_namespace: &str,
        _gateway_class_params: Option<&GatewayClassParameters>,
        _gateway_params: Option<&GatewayParameters>,
        _base_metadata: Option<ObjectMeta>,
    ) -> MergedGatewayConfiguration {
        // This is a simplified implementation - in a real system this would
        // merge parameters with proper precedence
        MergedGatewayConfiguration {
            deployment: Deployment::default(),
            service: Service::default(),
            config_map: ConfigMap::default(),
            image_repository: "ghcr.io/daniel-white/vale-gateway".to_string(),
            image_tag: "latest".to_string(),
            gateway_config: GatewayConfiguration::default(),
            open_telemetry: None,
        }
    }
}

/// Service that merges gateway configurations from class and instance parameters
pub struct GatewayConfigurationMergerService;

impl GatewayConfigurationMergerService {
    /// Create a receiver that provides merged gateway configurations
    pub fn merged_configurations(
        task_builder: &TaskBuilder,
        gateways_rx: &Receiver<Option<Arc<Gateways>>>,
    ) -> Receiver<GatewayResourceConfigurations> {
        let (tx, rx) = signal("merged_gateway_configurations");
        let gateways_rx = gateways_rx.clone();

        task_builder
            .new_task("merge_gateway_configurations")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready(gateways_opt) = await_ready!(gateways_rx) {
                        let mut configurations = GatewayResourceConfigurations::new();

                        if let Some(gateways) = gateways_opt.as_ref() {
                            info!(
                                "Merging configurations for {} gateways",
                                gateways.instances().len()
                            );

                            for (
                                gateway_ref,
                                _gateway_class,
                                gateway_class_params,
                                gateway,
                                gateway_params,
                            ) in gateways.iter()
                            {
                                debug!("Merging configuration for gateway: {}", gateway_ref);

                                let merged_config = Self::merge_single_gateway(
                                    gateway_ref,
                                    &_gateway_class,
                                    &gateway_class_params,
                                    &gateway,
                                    &gateway_params,
                                );

                                let resource_config = Self::convert_to_resource_config(
                                    gateway_ref,
                                    &gateway,
                                    merged_config,
                                );

                                configurations.add(resource_config);
                            }
                        }

                        info!(
                            "Merged configurations for {} gateways",
                            configurations.len()
                        );
                        tx.set(configurations).await;
                    }

                    continue_on!(gateways_rx.changed());
                }
            });

        rx
    }

    /// Merge configuration for a single gateway instance
    fn merge_single_gateway(
        gateway_ref: &ObjectRef,
        _gateway_class: &Arc<gateway_api::apis::standard::gatewayclasses::GatewayClass>,
        gateway_class_params: &Arc<GatewayClassParameters>,
        gateway: &Arc<Gateway>,
        gateway_params: &Arc<GatewayParameters>,
    ) -> MergedGatewayConfiguration {
        let gateway_name = gateway_ref.name();
        let gateway_namespace = gateway_ref
            .namespace()
            .clone()
            .unwrap_or("default".to_string());

        // Extract base metadata from gateway infrastructure
        let base_metadata = gateway
            .spec
            .infrastructure
            .as_ref()
            .map(|infra| ObjectMeta {
                labels: infra.labels.clone(),
                annotations: infra.annotations.clone(),
                ..Default::default()
            });

        ConfigurationMerger::merge(
            gateway_name,
            &gateway_namespace,
            Some(gateway_class_params.as_ref()),
            Some(gateway_params.as_ref()),
            base_metadata,
        )
    }

    /// Convert merged configuration to resource configuration
    fn convert_to_resource_config(
        gateway_ref: &ObjectRef,
        gateway: &Arc<Gateway>,
        merged_config: MergedGatewayConfiguration,
    ) -> GatewayResourceConfiguration {
        let gateway_name = gateway_ref.name().to_string();
        let gateway_namespace = gateway_ref
            .namespace()
            .clone()
            .unwrap_or("default".to_string());

        let metadata = GatewayResourceMetadata::builder()
            .gateway_class_name(gateway.spec.gateway_class_name.clone())
            .managed(true)
            .build();

        let resource_config = GatewayResourceConfiguration::builder()
            .name(gateway_name)
            .namespace(gateway_namespace)
            .deployment(merged_config.deployment().clone())
            .service(merged_config.service().clone())
            .config_map(merged_config.config_map().clone())
            .image_repository(merged_config.image_repository().clone())
            .image_tag(merged_config.image_tag().clone())
            .gateway_config(merged_config.gateway_config().clone())
            .open_telemetry(merged_config.open_telemetry().clone())
            .metadata(metadata)
            .build();

        resource_config.with_common_labels()
    }
}

/// Create individual configuration receivers for specific resource types
pub struct GatewayResourceReceivers;

impl GatewayResourceReceivers {
    /// Create a receiver for deployment configurations
    pub fn deployments(
        task_builder: &TaskBuilder,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) -> Receiver<HashMap<ObjectRef, k8s_openapi::api::apps::v1::Deployment>> {
        let (tx, rx) = signal("gateway_deployments");
        let configurations_rx = configurations_rx.clone();

        task_builder
            .new_task("extract_gateway_deployments")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready(configurations) = await_ready!(configurations_rx) {
                        let deployments: HashMap<ObjectRef, _> = configurations
                            .iter()
                            .map(|(name, config)| {
                                let object_ref = ObjectRef::builder()
                                    .kind("Deployment")
                                    .name(name)
                                    .namespace(config.namespace().clone())
                                    .build();
                                (object_ref, config.deployment().clone())
                            })
                            .collect();

                        tx.set(deployments).await;
                    }

                    continue_on!(configurations_rx.changed());
                }
            });

        rx
    }

    /// Create a receiver for service configurations
    pub fn services(
        task_builder: &TaskBuilder,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) -> Receiver<HashMap<ObjectRef, k8s_openapi::api::core::v1::Service>> {
        let (tx, rx) = signal("gateway_services");
        let configurations_rx = configurations_rx.clone();

        task_builder
            .new_task("extract_gateway_services")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready(configurations) = await_ready!(configurations_rx) {
                        let services: HashMap<ObjectRef, _> = configurations
                            .iter()
                            .map(|(name, config)| {
                                let object_ref = ObjectRef::builder()
                                    .kind("Service")
                                    .name(name)
                                    .namespace(config.namespace().clone())
                                    .build();
                                (object_ref, config.service().clone())
                            })
                            .collect();

                        tx.set(services).await;
                    }

                    continue_on!(configurations_rx.changed());
                }
            });

        rx
    }

    /// Create a receiver for config map configurations
    pub fn config_maps(
        task_builder: &TaskBuilder,
        configurations_rx: &Receiver<GatewayResourceConfigurations>,
    ) -> Receiver<HashMap<ObjectRef, k8s_openapi::api::core::v1::ConfigMap>> {
        let (tx, rx) = signal("gateway_config_maps");
        let configurations_rx = configurations_rx.clone();

        task_builder
            .new_task("extract_gateway_config_maps")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready(configurations) = await_ready!(configurations_rx) {
                        let config_maps: HashMap<ObjectRef, _> = configurations
                            .iter()
                            .map(|(name, config)| {
                                let object_ref = ObjectRef::builder()
                                    .kind("ConfigMap")
                                    .name(&config.config_map_name())
                                    .namespace(config.namespace().clone())
                                    .build();
                                (object_ref, config.config_map().clone())
                            })
                            .collect();

                        tx.set(config_maps).await;
                    }

                    continue_on!(configurations_rx.changed());
                }
            });

        rx
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_merge_single_gateway() {
        // This would require setting up mock data structures
        // Implementation would depend on your testing framework
    }
}
