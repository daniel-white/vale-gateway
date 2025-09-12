use crate::gateways::resources::{GatewayResourceConfiguration, GatewayResourceConfigurations};
use crate::ipc::instances::InstanceRole;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::{ObjectRef, SyncObjectAction};
use crate::options::Options;
use crate::{sync_objects, watch_objects};
use gtmpl_derive::Gtmpl;
use k8s_openapi::api::core::v1::ConfigMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info};
use typed_builder::TypedBuilder;
use vg_core::gateways::Gateway;
use vg_core::ipc::IpcConfiguration;
use vg_core::sync::signal::Receiver;
use vg_core::task::Builder as TaskBuilder;
use vg_core::{ReadyState, await_ready, continue_after};

const TEMPLATE: &str = include_str!("./templates/gateway_configmap.kubernetes-helm-yaml");

#[derive(Clone, TypedBuilder, Debug, Gtmpl)]
struct TemplateValues {
    #[builder(setter(into))]
    gateway_name: String,
    #[builder(setter(into))]
    config_yaml: String,
}

/// Synchronizer for Kubernetes `ConfigMaps`
#[derive(Debug, Clone)]
pub struct ConfigMapSynchronizer;

impl ConfigMapSynchronizer {
    pub fn new() -> Self {
        Self
    }

    /// Sync all configmaps from gateway configurations using templates
    pub fn sync_all_configmaps(
        &self,
        task_builder: &TaskBuilder,
        options: Arc<Options>,
        kube_client_rx: Receiver<KubeClientCell>,
        instance_role_rx: Receiver<InstanceRole>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
        ipc_config: Arc<IpcConfiguration>,
    ) {
        let (tx, current_refs_rx) = sync_objects!(
            options,
            task_builder,
            ConfigMap,
            kube_client_rx,
            instance_role_rx,
            TemplateValues,
            TEMPLATE
        );

        ConfigMapSynchronizer::generate_configmaps(
            task_builder,
            tx,
            current_refs_rx,
            configurations_rx,
            ipc_config,
        );
    }

    fn generate_configmaps(
        task_builder: &TaskBuilder,
        sync_tx: UnboundedSender<SyncObjectAction<TemplateValues, ConfigMap>>,
        current_refs_rx: Receiver<HashSet<ObjectRef>>,
        configurations_rx: Receiver<GatewayResourceConfigurations>,
        ipc_config: Arc<IpcConfiguration>,
    ) {
        task_builder
            .new_task("generate_gateway_configmaps")
            .spawn(async move {
                loop {
                    if let ReadyState::Ready((configurations, current_configmap_refs)) =
                        await_ready!(configurations_rx, current_refs_rx)
                    {
                        info!("Reconciling Gateway ConfigMaps");

                        let desired_configmaps = ConfigMapSynchronizer::expand_configurations(
                            configurations,
                            ipc_config.clone(),
                        );
                        let desired_configmap_refs: HashSet<_> = desired_configmaps
                            .iter()
                            .map(|state| state.configmap_ref.clone())
                            .collect();

                        // Delete removed configmaps
                        let deleted_refs =
                            current_configmap_refs.difference(&desired_configmap_refs);
                        for deleted_ref in deleted_refs {
                            if let Err(err) =
                                sync_tx.send(SyncObjectAction::Delete(deleted_ref.clone()))
                            {
                                error!("Failed to send delete action: {}", err);
                            }
                        }

                        // Create/update desired configmaps
                        for configmap_state in desired_configmaps {
                            if let Some(template_values) = configmap_state.template_values {
                                if let Err(err) = sync_tx.send(SyncObjectAction::Upsert(
                                    configmap_state.configmap_ref,
                                    configmap_state.gateway_ref,
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
        ipc_config: Arc<IpcConfiguration>,
    ) -> Vec<ConfigMapState> {
        configurations
            .iter()
            .map(|(name, config)| {
                let gateway_ref = ObjectRef::builder()
                    .kind("Gateway")
                    .name(name)
                    .namespace(config.namespace().clone())
                    .build();

                let configmap_ref = ObjectRef::builder()
                    .kind("ConfigMap")
                    .name(config.config_map_name())
                    .namespace(config.namespace().clone())
                    .build();

                let template_values = ConfigMapSynchronizer::generate_template_values(
                    name,
                    config,
                    ipc_config.clone(),
                );

                ConfigMapState {
                    gateway_ref,
                    configmap_ref,
                    template_values: Some(template_values),
                }
            })
            .collect()
    }

    fn generate_template_values(
        gateway_name: &str,
        config: &GatewayResourceConfiguration,
        ipc_config: Arc<IpcConfiguration>,
    ) -> TemplateValues {
        // Create Gateway configuration from the GatewayResourceConfiguration
        let gateway_config = ConfigMapSynchronizer::create_gateway_from_config(config, ipc_config);

        // Serialize the Gateway to YAML
        let config_yaml = match serde_yaml::to_string(&gateway_config) {
            Ok(yaml) => yaml,
            Err(err) => {
                error!(
                    "Failed to serialize Gateway configuration for {}: {}",
                    gateway_name, err
                );
                // Fallback to default configuration
                ConfigMapSynchronizer::generate_default_gateway_config()
            }
        };

        TemplateValues::builder()
            .gateway_name(gateway_name)
            .config_yaml(config_yaml)
            .build()
    }
}

#[derive(Debug)]
struct ConfigMapState {
    gateway_ref: ObjectRef,
    configmap_ref: ObjectRef,
    template_values: Option<TemplateValues>,
}

impl ConfigMapSynchronizer {
    /// Create a Gateway struct from `GatewayResourceConfiguration`
    fn create_gateway_from_config(
        _config: &GatewayResourceConfiguration,
        ipc_config: Arc<IpcConfiguration>,
    ) -> Gateway {
        // TODO: Convert the API GatewayConfiguration to core HttpListener
        // For now, we don't create an HTTP listener as the API configuration structure
        // is different from the core HttpListener structure and would require conversion logic
        let http_listener = None;

        Gateway::builder()
            .ipc(ipc_config)
            .http_listener(http_listener)
            .build()
    }

    /// Generate default gateway configuration
    fn generate_default_gateway_config() -> String {
        r#"# Vale Gateway Configuration
logLevel: Info
listeners:
  http:
    filters: []
instrumentation:
  openTelemetry:
    exporter:
      endpoint: "http://jaeger:14268/api/traces"
    sampling:
      samplingType: TraceIdRatioBased
      traceIdRatioBased:
        ratio: 0.1
"#
        .to_string()
    }
}

impl Default for ConfigMapSynchronizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configmap_synchronizer_creation() {
        let _sync = ConfigMapSynchronizer::new();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_configmap_synchronizer_default() {
        let _sync = ConfigMapSynchronizer::default();
        // Just test creation doesn't panic
    }

    #[test]
    fn test_generate_default_gateway_config() {
        let config = ConfigMapSynchronizer::generate_default_gateway_config();

        assert!(config.contains("logLevel: Info"));
        assert!(config.contains("listeners:"));
        assert!(config.contains("instrumentation:"));

        // Should be valid YAML
        assert!(serde_yaml::from_str::<serde_yaml::Value>(&config).is_ok());
    }

    #[test]
    fn test_template_values_creation() {
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .config_yaml("logLevel: Info")
            .build();

        assert_eq!(template_values.gateway_name, "test-gateway");
        assert_eq!(template_values.config_yaml, "logLevel: Info");
    }

    #[test]
    fn test_create_gateway_from_config() {
        use crate::gateways::resources::GatewayResourceConfiguration;
        use k8s_openapi::api::apps::v1::Deployment;
        use k8s_openapi::api::core::v1::{ConfigMap, Service};
        use vg_api::v1alpha1::GatewayConfiguration;

        let config = GatewayResourceConfiguration::builder()
            .name("test-gateway")
            .namespace("default")
            .deployment(Deployment::default())
            .service(Service::default())
            .config_map(ConfigMap::default())
            .image_repository("vale-gateway")
            .image_tag("latest")
            .gateway_config(GatewayConfiguration::default())
            .open_telemetry(None)
            .build();

        let ipc_config = Arc::new(
            IpcConfiguration::builder()
                .addr(std::net::SocketAddr::from(([127, 0, 0, 1], 8080)))
                .build(),
        );

        let gateway = ConfigMapSynchronizer::create_gateway_from_config(&config, ipc_config);

        // Verify the gateway can be serialized to YAML
        let yaml_result = serde_yaml::to_string(&gateway);
        assert!(yaml_result.is_ok());

        let yaml = yaml_result.unwrap();
        assert!(yaml.contains("version: v1alpha1"));
        assert!(yaml.contains("ipc:"));
    }
}
