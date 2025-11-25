use super::{AccessControlFilterEffect, AccessControlFilterSpec};
use crate::resources::AccessControlFilterRef;
use std::collections::HashSet;
use thiserror::Error;
use vg_config::http::filter::access_control::{
    AccessControlEffect, AccessControlFilter, AccessControlFilterRef as AccessControlFilterRefConfig,
};
use vg_core::net::IpRef;

impl From<AccessControlFilterRef> for AccessControlFilterRefConfig {
    fn from(value: AccessControlFilterRef) -> Self {
        value.to_string().into()
    }
}

#[derive(Debug, Error)]
pub enum AccessControlFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Must specify at least one client")]
    MissingClients,
}

impl TryFrom<&AccessControlFilterSpec> for AccessControlFilter {
    type Error = AccessControlFilterConversionError;

    fn try_from(value: &AccessControlFilterSpec) -> Result<Self, Self::Error> {
        let effect: AccessControlEffect = value.effect.clone().into();

        let clients: Vec<IpRef> = {
            let mut clients: HashSet<IpRef> = HashSet::new();

            for client_ip in &value.clients.ips {
                clients.insert(client_ip.into());
            }

            for client_ip_range in &value.clients.ip_ranges {
                clients.insert(client_ip_range.into());
            }

            clients.into_iter().collect()
        };

        if clients.is_empty() {
            return Err(AccessControlFilterConversionError::MissingClients);
        }

        let filter = Self::builder().effect(effect).clients(clients).build();

        Ok(filter)
    }
}

impl From<AccessControlFilterEffect> for AccessControlEffect {
    fn from(value: AccessControlFilterEffect) -> Self {
        match value {
            AccessControlFilterEffect::Allow => Self::Allow,
            AccessControlFilterEffect::Deny => Self::Deny,
        }
    }
}
