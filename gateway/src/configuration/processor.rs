use crate::configuration::{BackendConfiguration, RoutingConfiguration};
use futures::future::join_all;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::arc_watch::Sender;
use vg_core::sync::observable::Observable;
use vg_rpc_client::api::ApiClient;
use vg_rpc_client::events::Event;

#[derive(TypedBuilder)]
pub struct ConfigurationProcessor {
    api_client: ApiClient,
    backends: Observable<BackendConfiguration>,
    routing: Observable<RoutingConfiguration>,
}

impl ConfigurationProcessor {
    pub async fn handle(&self, event: Event) {
        match event {
            Event::Initialize => {
                let _ = self.init().await;
            }
            Event::ListenerChanged => {
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
        let listener = self
            .api_client
            .listener()
            .await
            .inspect_err(|err| println!("listener error: {:?}", err))
            .map_err(|_| ())?;
        let routes = self
            .api_client
            .routes(listener.route_refs().as_slice())
            .await
            .map_err(|_| ())?;
        let shared_filters = self
            .api_client
            .shared_filters(listener.shared_filter_refs().as_slice())
            .await
            .map_err(|_| ())?;
        let backends = self
            .api_client
            .backends(listener.backend_refs().as_slice())
            .await
            .map_err(|_| ())?;

        self.routing.update(|_| {
            let routes = routes
                .iter()
                .map(|route| (route.ref_(), route.clone()))
                .collect();
            let shared_filters = shared_filters
                .iter()
                .map(|filter| (filter.ref_(), filter.clone()))
                .collect();

            RoutingConfiguration::builder()
                .listener(Some(listener.clone()))
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

        self.routing.update(|routing| {
            let mut routes = routing.routes().clone();
            routes.insert(route.ref_(), route.clone());
    
            RoutingConfiguration::builder()
                .listener(routing.listener().clone())
                .routes(routes)
                .shared_filters(routing.shared_filters().clone())
                .build()
        });

        Ok(())
    }

    async fn sync_shared_filter(&self, filter_ref: SharedFilterRef) -> Result<(), ()> {
        let shared_filter = self
            .api_client
            .shared_filter(&filter_ref)
            .await
            .map_err(|_| ())?;

        self.routing.update(|routing| {
            let mut shared_filters = routing.shared_filters().clone();
            shared_filters.insert(shared_filter.ref_(), shared_filter.clone());
    
            RoutingConfiguration::builder()
                .listener(routing.listener.clone())
                .routes(routing.routes.clone())
                .shared_filters(shared_filters)
                .build()
        });

        Ok(())
    }

    async fn sync_backend(&self, backend_ref: BackendRef) -> Result<(), ()> {
        let backend = self
            .api_client
            .backend(&backend_ref)
            .await
            .map_err(|_| ())?;

         self.backends.update(|backends|{
        let mut backends = backends.backends().clone();
        backends.insert(backend.ref_(), backend.clone());

        BackendConfiguration::builder().backends(backends).build()
        });
            
        Ok(())
    }
}
