use crate::configuration::{BackendConfiguration};
use enumflags2::BitFlags;
use getset::{CloneGetters, Getters};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend as BackendConfig, BackendRef};
use vg_core::net::topology::{TopologyLocation, TopologyLocationMatch};
use vg_core::sync::arc_watch::{Receiver, Sender, channel};
use vg_core::sync::handles::{Handle, handles};

#[derive(TypedBuilder, Clone, Debug, Getters, CloneGetters)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct Backend {
    #[getset(get_clone = "pub")]
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
            .ref_(value.ref_())
            .endpoints(endpoints)
            .build()
    }
}

#[derive(Debug, Default, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct Backends {
    backends: HashMap<BackendRef, Arc<Backend>>
}

impl From<(&TopologyLocation, &BackendConfiguration)> for Backends {
    fn from((current_location, value): (&TopologyLocation, &BackendConfiguration)) -> Self {
        let backends = value
            .backends()
            .iter()
            .map(|(ref_, backend)| {
                let backend: Backend = (current_location, backend.as_ref()).into();
                (ref_.clone(), Arc::new(backend))
            })
            .collect();

        Self::builder().backends(backends).build()
    }
}

#[derive(TypedBuilder)]
pub struct BackendConfiguratorOptions {
    current_location: Receiver<TopologyLocation>,
    backend_configuration: Receiver<BackendConfiguration>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct BackendConfigurator {
    current_location: Receiver<TopologyLocation>,
    backend_configuration: Receiver<BackendConfiguration>,
    backends: Sender<Backends>,
}

impl From<BackendConfiguratorOptions> for BackendConfigurator {
    fn from(value: BackendConfiguratorOptions) -> Self {
        let (backends, _) = channel();

        Self::builder()
            .current_location(value.current_location)
            .backend_configuration(value.backend_configuration)
            .backends(backends)
            .build()
    }
}

impl BackendConfigurator {
    pub fn backends(&self) -> Receiver<Backends> {
        self.backends.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            let mut current_location = self.current_location;
            let mut backend_configuration = self.backend_configuration;
            loop {
                let backends =  {
                    let current_location = current_location.current().unwrap_or_default();
                    let source_backends = backend_configuration.current().unwrap_or_default();
                    (current_location.as_ref(), source_backends.as_ref()).into()
                };
                let _ = self.backends.send(Arc::new(backends));

                select! {
                    _ = current_location.changed() => {
                        continue;
                    }
                    _ = backend_configuration.changed() => {
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
