use crate::configuration::{BackendConfiguration, GatewayConfiguration};
use futures::future::join_all;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::gateway::{Gateway, GatewayRef};
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::arc_watch::Sender;
use vg_core::sync::observable::Observable;
use vg_rpc_client::api::ApiClient;
use vg_rpc_client::events::Event;

#[derive(TypedBuilder)]
pub struct ConfigurationProcessor {
    api_client: ApiClient,
    backends: Observable<BackendConfiguration>,
    gateway: Observable<GatewayConfiguration>,
    gateway_ref: GatewayRef,
}

impl ConfigurationProcessor {
    pub async fn handle(&self, event: Event) {
        match event {
            Event::Initialize => {
                let _ = self.init().await;
            }
            Event::GatewayChanged => {
                let _ = self.sync_all().await;
            }
            Event::RouteChanged(route_ref) => {
                let _ = self.sync_route(route_ref).await;
            }
            Event::BackendChanged(backend_ref) => {
                let _ = self.sync_backend(backend_ref).await;
            }
            Event::SharedFilterChanged(filter_ref) => {
                let _ = self.sync_shared_filter(filter_ref).await;
            }
        };
    }

    pub async fn init(&self) {
        let _ = self.sync_all().await;
    }

    async fn sync_all(&self) -> Result<(), ()> {
        // Get the gateway with all embedded resources using the configured gateway reference
        let gateway = self
            .api_client
            .gateway(&self.gateway_ref)
            .await
            .inspect_err(|err| println!("gateway error: {:?}", err))
            .map_err(|_| ())?;

        // Get routes referenced by the gateway's listeners
        let route_refs: Vec<_> = gateway
            .listeners()
            .iter()
            .flat_map(|listener| listener.route_refs().iter().cloned())
            .collect();

        let routes = self.api_client.routes(&route_refs).await.map_err(|_| ())?;

        // Get shared filters referenced by the gateway and its listeners
        let shared_filter_refs: Vec<_> = gateway.shared_filter_refs().clone();

        let shared_filters = self
            .api_client
            .shared_filters(&shared_filter_refs)
            .await
            .map_err(|_| ())?;

        // Collect backend references from all routes
        let backend_refs: Vec<_> = routes
            .iter()
            .flat_map(|route| {
                route.rules().iter().flat_map(|rule| {
                    rule.backend_refs()
                        .iter()
                        .map(|backend_ref| backend_ref.backend_ref().clone())
                })
            })
            .collect();

        let backends = self.api_client.backends(&backend_refs).await.map_err(|_| ())?;

        self.gateway.update(|_| {
            let routes = routes.iter().map(|route| (route.ref_(), route.clone())).collect();
            let shared_filters = shared_filters
                .iter()
                .map(|filter| (filter.ref_(), filter.clone()))
                .collect();

            GatewayConfiguration::builder()
                .gateway(Some(gateway.clone()))
                .routes(routes)
                .shared_filters(shared_filters)
                .build()
        });

        self.backends.update(|_| {
            let backends = backends
                .iter()
                .map(|backend| (backend.ref_(), backend.clone()))
                .collect();

            BackendConfiguration::builder().backends(backends).build()
        });

        Ok(())
    }

    async fn sync_route(&self, route_ref: RouteRef) -> Result<(), ()> {
        let route = self.api_client.route(&route_ref).await.map_err(|_| ())?;

        self.gateway.update(|config| {
            let mut routes = config.routes().clone();
            routes.insert(route.ref_(), route.clone());

            GatewayConfiguration::builder()
                .gateway(config.gateway().clone())
                .routes(routes)
                .shared_filters(config.shared_filters().clone())
                .build()
        });

        Ok(())
    }

    async fn sync_shared_filter(&self, filter_ref: SharedFilterRef) -> Result<(), ()> {
        let shared_filter = self.api_client.shared_filter(&filter_ref).await.map_err(|_| ())?;

        self.gateway.update(|config| {
            let mut shared_filters = config.shared_filters().clone();
            shared_filters.insert(shared_filter.ref_(), shared_filter.clone());

            GatewayConfiguration::builder()
                .gateway(config.gateway().clone())
                .routes(config.routes().clone())
                .shared_filters(shared_filters)
                .build()
        });

        Ok(())
    }

    async fn sync_backend(&self, backend_ref: BackendRef) -> Result<(), ()> {
        let backend = self.api_client.backend(&backend_ref).await.map_err(|_| ())?;

        self.backends.update(|backends| {
            let mut backends = backends.backends().clone();
            backends.insert(backend.ref_(), backend.clone());

            BackendConfiguration::builder().backends(backends).build()
        });

        Ok(())
    }
}
