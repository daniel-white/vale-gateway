use crate::configuration::{Receiver, SourceBackendConfiguration};
use async_stm::{TVar, atomically};
use enumflags2::BitFlags;
use getset::{CloneGetters, Getters};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend as BackendConfig, BackendRef};
use vg_core::net::topology::{TopologyLocation, TopologyLocationMatch};
use vg_core::sync::arc_watch::{Sender, channel};
use vg_core::sync::handles::{Handle, handles};

#[derive(TypedBuilder, Clone, Debug, Getters, CloneGetters)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
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
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct BackendConfiguration {
    backends: HashMap<Arc<BackendRef>, Arc<BackendAddresses>>,
}

impl From<(&TopologyLocation, &SourceBackendConfiguration)> for BackendConfiguration {
    fn from((current_location, value): (&TopologyLocation, &SourceBackendConfiguration)) -> Self {
        let backends = value
            .backends()
            .iter()
            .map(|(ref_, backend)| {
                let backend: BackendAddresses = (current_location, backend.as_ref()).into();
                (ref_.clone(), Arc::new(backend))
            })
            .collect();

        Self::builder().backends(backends).build()
    }
}

#[derive(TypedBuilder)]
pub struct BackendConfiguratorOptions {
    current_location: Receiver<TopologyLocation>,
    backends: Receiver<SourceBackendConfiguration>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct BackendConfigurator {
    current_location: Receiver<TopologyLocation>,
    backends: Receiver<SourceBackendConfiguration>,
    configuration: TVar<BackendConfiguration>,
    configuration_tx: Sender<BackendConfiguration>,
}

impl From<BackendConfiguratorOptions> for BackendConfigurator {
    fn from(value: BackendConfiguratorOptions) -> Self {
        let (tx, _) = channel();

        Self::builder()
            .current_location(value.current_location)
            .backends(value.backends)
            .configuration(Default::default())
            .configuration_tx(tx)
            .build()
    }
}

impl BackendConfigurator {
    pub fn backends(&self) -> Receiver<BackendConfiguration> {
        self.configuration_tx.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut current_location = self.current_location;
        let mut backends = self.backends;
        let configuration_t = self.configuration;
        let configuration_tx = self.configuration_tx;

        spawn(async move {
            loop {
                let configuration = atomically(|| {
                    let current_location = current_location.current().unwrap_or_default();
                    let source_backends = backends.current().unwrap_or_default();
                    let configuration =
                        (current_location.as_ref(), source_backends.as_ref()).into();
                    configuration_t.write(configuration)?;

                    configuration_t.read()
                })
                .await;
                let _ = configuration_tx.send(configuration);

                select! {
                    _ = current_location.changed() => {
                        continue;
                    }
                    _ = backends.changed() => {
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
