use getset::{CloneGetters, Getters};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::{GatewayConfiguration, GatewayInstrumentationOpenTelemetry};

/// Complete configuration for a gateway instance including all Kubernetes resources
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder)]
pub struct GatewayResourceConfiguration {
    /// The gateway instance name
    #[getset(get = "pub")]
    #[builder(setter(into))]
    name: String,

    /// The gateway instance namespace
    #[getset(get = "pub")]
    #[builder(setter(into))]
    namespace: String,

    /// Kubernetes Deployment configuration
    #[getset(get = "pub")]
    deployment: Deployment,

    /// Kubernetes Service configuration
    #[getset(get = "pub")]
    service: Service,

    /// Kubernetes `ConfigMap` configuration
    #[getset(get = "pub")]
    config_map: ConfigMap,

    /// Container image repository
    #[getset(get = "pub")]
    #[builder(setter(into))]
    image_repository: String,

    /// Container image tag
    #[getset(get = "pub")]
    #[builder(setter(into))]
    image_tag: String,

    /// Gateway-specific configuration
    #[getset(get = "pub")]
    gateway_config: GatewayConfiguration,

    /// OpenTelemetry configuration if enabled
    #[getset(get = "pub")]
    open_telemetry: Option<GatewayInstrumentationOpenTelemetry>,

    /// Additional metadata for the gateway
    #[getset(get = "pub")]
    #[builder(default)]
    metadata: GatewayResourceMetadata,
}

/// Additional metadata for gateway resources
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, Getters, TypedBuilder)]
pub struct GatewayResourceMetadata {
    /// Labels to apply to all resources
    #[getset(get = "pub")]
    #[builder(default)]
    labels: HashMap<String, String>,

    /// Annotations to apply to all resources
    #[getset(get = "pub")]
    #[builder(default)]
    annotations: HashMap<String, String>,

    /// The gateway class name this instance belongs to
    #[getset(get = "pub")]
    #[builder(default, setter(into))]
    gateway_class_name: String,

    /// Whether this gateway is managed by the controller
    #[getset(get = "pub")]
    #[builder(default)]
    managed: bool,
}

impl GatewayResourceConfiguration {
    /// Get the full container image reference
    pub fn container_image(&self) -> String {
        format!("{}:{}", self.image_repository(), self.image_tag())
    }

    /// Check if OpenTelemetry is enabled
    pub fn is_otel_enabled(&self) -> bool {
        self.open_telemetry().is_some()
    }

    /// Get the deployment name (same as gateway name)
    pub fn deployment_name(&self) -> &str {
        self.name()
    }

    /// Get the service name (same as gateway name)
    pub fn service_name(&self) -> &str {
        self.name()
    }

    /// Get the config map name
    pub fn config_map_name(&self) -> String {
        format!("{}-config", self.name())
    }

    /// Apply common labels to all resources
    pub fn with_common_labels(self) -> Self {
        let name = self.name().clone();
        let common_labels = [
            ("app.kubernetes.io/name", "vale-gateway"),
            ("app.kubernetes.io/instance", name.as_str()),
            ("app.kubernetes.io/component", "gateway"),
            ("app.kubernetes.io/managed-by", "vale-gateway-controller"),
        ];

        Self {
            metadata: {
                let mut metadata = self.metadata.clone();
                for (key, value) in &common_labels {
                    metadata
                        .labels
                        .insert((*key).to_string(), (*value).to_string());
                }
                metadata
            },
            deployment: {
                let mut deployment = self.deployment.clone();
                if let Some(labels) = &mut deployment.metadata.labels {
                    for (key, value) in &common_labels {
                        labels.insert((*key).to_string(), (*value).to_string());
                    }
                }
                deployment
            },
            service: {
                let mut service = self.service.clone();
                if let Some(labels) = &mut service.metadata.labels {
                    for (key, value) in &common_labels {
                        labels.insert((*key).to_string(), (*value).to_string());
                    }
                }
                service
            },
            config_map: {
                let mut config_map = self.config_map.clone();
                if let Some(labels) = &mut config_map.metadata.labels {
                    for (key, value) in &common_labels {
                        labels.insert((*key).to_string(), (*value).to_string());
                    }
                }
                config_map
            },
            ..self
        }
    }

    /// Convert the gateway configuration to the core gateway type using the provided IPC configuration
    pub fn to_core_gateway(
        &self,
        ipc_config: std::sync::Arc<vg_core::ipc::IpcConfiguration>,
    ) -> Result<vg_core::gateways::Gateway, anyhow::Error> {
        use vg_core::gateways::Gateway;

        // Convert the gateway configuration to HTTP listener if present
        let http_listener = if let Some(listeners) = &self.gateway_config.listeners {
            if let Some(_http_config) = &listeners.http {
                // TODO: Implement proper conversion from API GatewayListenerHttp to core HttpListener
                // For now, we skip the HTTP listener conversion since the types are incompatible
                // This will need to be implemented properly when the conversion logic is added
                None
            } else {
                None
            }
        } else {
            None
        };

        Ok(Gateway::builder()
            .ipc(ipc_config)
            .http_listener(http_listener)
            .build())
    }
}

/// Collection of gateway resource configurations
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct GatewayResourceConfigurations {
    /// Map of gateway name to its configuration
    pub gateways: HashMap<String, GatewayResourceConfiguration>,
}

impl GatewayResourceConfigurations {
    /// Create a new empty collection
    pub fn new() -> Self {
        Self {
            gateways: HashMap::new(),
        }
    }

    /// Add a gateway configuration
    pub fn add(&mut self, config: GatewayResourceConfiguration) {
        self.gateways.insert(config.name.clone(), config);
    }

    /// Remove a gateway configuration
    pub fn remove(&mut self, name: &str) -> Option<GatewayResourceConfiguration> {
        self.gateways.remove(name)
    }

    /// Get a gateway configuration by name
    pub fn get(&self, name: &str) -> Option<&GatewayResourceConfiguration> {
        self.gateways.get(name)
    }

    /// Get a mutable gateway configuration by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut GatewayResourceConfiguration> {
        self.gateways.get_mut(name)
    }

    /// Iterate over all gateway configurations
    pub fn iter(&self) -> impl Iterator<Item = (&String, &GatewayResourceConfiguration)> {
        self.gateways.iter()
    }

    /// Get the number of gateway configurations
    pub fn len(&self) -> usize {
        self.gateways.len()
    }

    /// Check if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.gateways.is_empty()
    }

    /// Get all deployment configurations
    pub fn deployments(&self) -> impl Iterator<Item = &Deployment> {
        self.gateways.values().map(|config| &config.deployment)
    }

    /// Get all service configurations
    pub fn services(&self) -> impl Iterator<Item = &Service> {
        self.gateways.values().map(|config| &config.service)
    }

    /// Get all config map configurations
    pub fn config_maps(&self) -> impl Iterator<Item = &ConfigMap> {
        self.gateways.values().map(|config| &config.config_map)
    }

    /// Convert all gateway configurations to core gateway types
    pub fn to_core_gateways(
        &self,
        ipc_config: std::sync::Arc<vg_core::ipc::IpcConfiguration>,
    ) -> Result<HashMap<String, vg_core::gateways::Gateway>, anyhow::Error> {
        let mut core_gateways = HashMap::new();
        for (name, config) in &self.gateways {
            core_gateways.insert(name.clone(), config.to_core_gateway(ipc_config.clone())?);
        }
        Ok(core_gateways)
    }
}
