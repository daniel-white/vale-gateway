use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use async_stm::{atomically, TVar};
use enumflags2::BitFlags;
use getset::{CloneGetters, Getters};
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_core::net::topology::{TopologyLocation, TopologyLocationMatch};
use vg_core::sync::handles::{handles, Handle};
use crate::configuration::{SourceBackendConfiguration, Receiver};
use vg_config::http::backend::{Backend as BackendConfig, BackendRef};
use vg_core::sync::arc_watch::{channel, Sender};

#[derive(TypedBuilder, Clone, Debug, Getters, CloneGetters)]
pub struct BackendAddresses {
    #[getset(get_clone = "pub")]
    ref_: Arc<BackendRef>,

    endpoints: HashMap<BitFlags<TopologyLocationMatch>, HashSet<IpAddr>>,
}


impl BackendAddresses {
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

impl From<(&TopologyLocation, &BackendConfig)> for BackendAddresses {
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
            .ref_(Arc::new(value.ref_()))
            .endpoints(endpoints)
            .build()
    }
}

#[derive(Debug, Default, Clone, TypedBuilder)]
pub struct BackendConfiguration {
    backends: HashMap<Arc<BackendRef>, Arc<BackendAddresses>>
}

impl From<(&TopologyLocation, &SourceBackendConfiguration)> for BackendConfiguration {
    fn from((current_location, value): (&TopologyLocation, &SourceBackendConfiguration)) -> Self {
        let backends = value.backends().iter().map(|(ref_, backend)| {
            let backend: BackendAddresses = (current_location, backend.as_ref()).into();
            (ref_.clone(), Arc::new(backend))
        }).collect();
        
        Self::builder()
            .backends(backends)
            .build()
    }
}

#[derive(TypedBuilder)]
pub struct BackendConfiguratorOptions {
    current_location_rx: Receiver<TopologyLocation>,
    source_backends_rx: Receiver<SourceBackendConfiguration>
}


#[derive(TypedBuilder)]
pub struct BackendConfigurator {
    current_location_rx: Receiver<TopologyLocation>,
    source_backends_rx: Receiver<SourceBackendConfiguration>,
    configuration: TVar<BackendConfiguration>,
    configuration_tx: Sender<BackendConfiguration>,
    configuration_rx: Receiver<BackendConfiguration>
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
    pub fn backends(&self) -> Receiver<BackendConfiguration> {
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