use crate::infra::{TopologyLocation, TopologyLocationMatch};
use enumflags2::BitFlags;
use getset::{CopyGetters, Getters};
use itertools::Itertools;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_core::http::routes::backends::HttpRouteBackend;

#[derive(Getters, CopyGetters, Debug, PartialEq, Eq)]
pub struct HttpRouteRuleBackend {
    #[getset(get_copy = "pub")]
    weight: i32,

    #[getset(get = "pub")]
    endpoints: HashMap<BitFlags<TopologyLocationMatch>, Vec<HttpRouteRuleBackendEndpoint>>,
}

impl HttpRouteRuleBackend {
    pub fn builder(location: Arc<TopologyLocation>) -> HttpRouteRuleBackendBuilder {
        HttpRouteRuleBackendBuilder {
            location,
            weight: 0,
            endpoints: Vec::new(),
        }
    }

    pub fn from(backend: &HttpRouteBackend, location: Arc<TopologyLocation>) -> Self {
        let mut builder = Self::builder(location);

        if let Some(weight) = backend.weight() {
            builder.with_weight(weight);
        }

        for endpoint in backend.endpoints() {
            let location = TopologyLocation::builder()
                .node(endpoint.node().clone())
                .zone(endpoint.zone().clone())
                .build();

            builder.add_endpoint(*endpoint.addr(), location);
        }

        builder.build()
    }

    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }
}

pub struct HttpRouteRuleBackendBuilder {
    location: Arc<TopologyLocation>,
    weight: i32,
    endpoints: Vec<(TopologyLocation, HttpRouteRuleBackendEndpoint)>,
}

impl HttpRouteRuleBackendBuilder {
    pub fn build(self) -> HttpRouteRuleBackend {
        // Apply the port to all endpoints if not already set
        let endpoints: HashMap<_, _> = self
            .endpoints
            .into_iter()
            .map(|(endpoint_location, endpoint)| {
                let score = TopologyLocationMatch::matches(&self.location, &endpoint_location);
                let score = if score.contains(TopologyLocationMatch::Node) {
                    BitFlags::from(TopologyLocationMatch::Node)
                } else if score.contains(TopologyLocationMatch::Zone) {
                    BitFlags::from(TopologyLocationMatch::Zone)
                } else {
                    BitFlags::empty()
                };
                (score, endpoint)
            })
            .into_group_map();

        HttpRouteRuleBackend {
            weight: self.weight,
            endpoints,
        }
    }

    pub fn with_weight(&mut self, weight: i32) -> &mut Self {
        self.weight = weight;
        self
    }

    pub fn add_endpoint(&mut self, addr: SocketAddr, location: TopologyLocation) -> &mut Self {
        let endpoint = HttpRouteRuleBackendEndpoint::builder().addr(addr).build();
        self.endpoints.push((location, endpoint));
        self
    }
}

#[derive(CopyGetters, Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpRouteRuleBackendEndpoint {
    #[getset(get_copy = "pub")]
    addr: SocketAddr,
}
