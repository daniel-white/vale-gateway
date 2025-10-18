use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use async_stm::{atomically, TVar};
use enumflags2::BitFlags;
use getset::Getters;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_core::net::topology::{TopologyLocation, TopologyLocationMatch};
use vg_core::sync::handles::{handles, Handle};
use crate::configuration::{SourceBackendConfiguration, ConfigurationWatch};
use vg_config::http::backend::{Backend as BackendConfig, BackendRef};
use vg_core::configuration::watch::{channel, ConfigurationSender};

#[derive(TypedBuilder, Clone, Debug, Getters)]
pub struct Backend {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    ref_: BackendRef,

    endpoints: HashMap<BitFlags<TopologyLocationMatch>, HashSet<IpAddr>>,
}


impl Backend {
    pub fn endpoints_matching(&self, location_match: TopologyLocationMatch) -> HashSet<IpAddr> {
        self.endpoints
            .iter()
            .filter_map(|(flags, addrs)| {
                if flags.contains(location_match) {
                    Some(addrs.iter().copied())
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }
}

impl From<(&TopologyLocation, &BackendConfig)> for Backend {
    fn from((current_location, value): (&TopologyLocation, &BackendConfig)) -> Self {
        let endpoints = value
            .endpoints()
            .iter()
            .map(|endpoint| {
                let location = TopologyLocation::builder()
                    .node(endpoint.node().clone())
                    .zone(endpoint.zone().clone())
                    .build();
                let location_match = TopologyLocationMatch::matches(current_location, &location);
                (
                    location_match,
                    HashSet::from_iter(endpoint.addrs().iter().cloned()),
                )
            })
            .collect();

        Self::builder()
            .ref_(value.ref_().clone())
            .endpoints(endpoints)
            .build()
    }
}

mod topology;

#[derive(Debug, Default, Clone, TypedBuilder)]
pub struct BackendConfiguration {
    backends: HashMap<BackendRef, Backend>
}

impl From<(&TopologyLocation, &SourceBackendConfiguration)> for BackendConfiguration {
    fn from((current_location, value): (&TopologyLocation, &SourceBackendConfiguration)) -> Self {
        let backends = value.backends().iter().map(|(ref_, backend)| {
            let backend: Backend = (current_location, backend).into();
            (ref_.clone(), backend)
        }).collect();
        
        Self::builder()
            .backends(backends)
            .build()
    }
}

#[derive(TypedBuilder)]
pub struct BackendConfiguratorOptions {
    current_location_rx: ConfigurationWatch<TopologyLocation>,
    source_backends_rx: ConfigurationWatch<SourceBackendConfiguration>
}


#[derive(TypedBuilder)]
pub struct BackendConfigurator {
    current_location_rx: ConfigurationWatch<TopologyLocation>,
    source_backends_rx: ConfigurationWatch<SourceBackendConfiguration>,
    configuration: TVar<BackendConfiguration>,
    configuration_tx: ConfigurationSender<BackendConfiguration>,
    configuration_rx: ConfigurationWatch<BackendConfiguration>
}

impl From<BackendConfiguratorOptions> for BackendConfigurator {
    fn from(value: BackendConfiguratorOptions) -> Self {
        let (tx, rx) = channel();
        
        Self::builder()
            .current_location_rx(value.current_location_rx)
            .source_backends_rx(value.source_backends_rx)
            .configuration(Default::default())
            .configuration_tx(tx)
            .configuration_rx(rx)
            .build()
    }
}

impl BackendConfigurator {
    pub fn backends(&self) -> ConfigurationWatch<BackendConfiguration> {
        self.configuration_rx.clone()
    }
    
    pub  fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut current_location_rx = self.current_location_rx;
        let mut source_backends_rx = self.source_backends_rx;
        let configuration_t = self.configuration;
        let configuration_tx = self.configuration_tx;
        
        spawn(async move {
            loop {
                let configuration = atomically(||  { 
                    let current_location = current_location_rx.current().unwrap_or_default();
                    let source_backends = source_backends_rx.current().unwrap_or_default();
                    let configuration = (current_location.as_ref(), source_backends.as_ref()).into();
                    configuration_t.write(configuration)?;
                    
                    configuration_t.read()
                } ).await;
                let _ = configuration_tx.send(configuration);
                
                select! {
                    _ = current_location_rx.changed() => {
                        continue;
                    }
                    _ = source_backends_rx.changed() => {
                        continue;
                    }
                    _ = stop_handle.stopped() => {
                        break;
                    }
                }
            }
        });
        
        handle
    }
}