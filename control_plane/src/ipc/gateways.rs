use crate::kubernetes::objects::ObjectRef;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use getset::Getters;
use std::sync::Arc;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_api::v1alpha1::GatewayConfiguration;

pub fn create_gateway_configuration_services()
-> (GatewayConfigurationReader, GatewayConfigurationManager) {
    let configurations = Arc::new(DashMap::new());
    (
        GatewayConfigurationReader::builder()
            .configurations(configurations.clone())
            .build(),
        GatewayConfigurationManager::builder()
            .configurations(configurations)
            .build(),
    )
}

#[derive(Debug, Clone, TypedBuilder, Getters)]
pub struct GatewayConfigurationReader {
    #[getset(get = "pub")]
    configurations: Arc<DashMap<ObjectRef, String>>,
}

impl GatewayConfigurationReader {
    pub fn exists(&self, gateway_ref: &ObjectRef) -> bool {
        self.configurations.contains_key(gateway_ref)
    }

    pub fn get_configuration_yaml(
        &'_ self,
        gateway_ref: &ObjectRef,
    ) -> Option<Ref<'_, ObjectRef, String>> {
        self.configurations.get(gateway_ref)
    }
}

#[derive(Debug, Clone, TypedBuilder, Getters)]
pub struct GatewayConfigurationManager {
    #[getset(get = "pub")]
    configurations: Arc<DashMap<ObjectRef, String>>,
}

#[derive(Debug, Error)]
pub enum GatewayConfigurationManagerInsertError {
    #[error("Failed to serialize configuration to YAML")]
    Serialize(#[from] serde_yaml::Error),
}

impl GatewayConfigurationManager {
    pub fn try_insert(
        &self,
        gateway_ref: ObjectRef,
        configuration: GatewayConfiguration,
    ) -> Result<(), GatewayConfigurationManagerInsertError> {
        let yaml = serde_yaml::to_string(&configuration)?;
        self.configurations.insert(gateway_ref, yaml);
        Ok(())
    }

    pub fn remove(&self, gateway_ref: &ObjectRef) -> bool {
        self.configurations.remove(gateway_ref).is_some()
    }
}
