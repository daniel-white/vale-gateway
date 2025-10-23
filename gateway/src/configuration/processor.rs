use crate::configuration::{SourceBackendConfiguration, SourceRoutingConfiguration};
use async_stm::{atomically, TVar};
use futures::future::join_all;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::arc_watch::Sender;
use vg_rpc_client::events::Event;
use vg_rpc_client::api::ApiClient;
use vg_rpc_client::api::error::ApiClientError;

#[derive(TypedBuilder)]
pub struct ConfigurationProcessor {
    api_client: ApiClient,
    #[builder(default, setter(skip))]
    backends: TVar<SourceBackendConfiguration>,
    backends_tx: Sender<SourceBackendConfiguration>,
    #[builder(default, setter(skip))]
    routing: TVar<SourceRoutingConfiguration>,
    routing_tx: Sender<SourceRoutingConfiguration>,
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
        let listener = self.api_client.listener().await.map_err(|_| ())?;
        let routes = self.api_client.routes(listener.route_refs().as_slice()).await.map_err(|_| ())?;
        let shared_filters = self.api_client.shared_filters(listener.shared_filter_refs().as_slice()).await.map_err(|_| ())?;
        let backends = self.api_client.backends(listener.backend_refs().as_slice()).await.map_err(|_| ())?;

        let (routing, backends) = atomically(|| {
            let routes = routes
                .iter()
                .map(|route| (Arc::new(route.ref_()), route.clone()))
                .collect();
            let shared_filters = shared_filters
                .iter()
                .map(|filter| (Arc::new(filter.ref_()), filter.clone()))
                .collect();

            let routing = SourceRoutingConfiguration::builder()
                .listener(Some(listener.clone()))
                .routes(routes)
                .shared_filters(shared_filters)
                .build();

            self.routing.write(routing)?;

            let backends = backends
                .iter()
                .map(|backend| (Arc::new(backend.ref_()), backend.clone()))
                .collect();

            let backends = SourceBackendConfiguration::builder()
                .backends(backends)
                .build();

            self.backends.write(backends)?;

            Ok((self.routing.read()?, self.backends.read()?))
        })
        .await;

        let _ = self.routing_tx.send(routing);
        let _ = self.backends_tx.send(backends);

        Ok(())
    }

    async fn sync_route(&self, route_ref: RouteRef) -> Result<(), ()> {
        let route = self.api_client.route(&route_ref).await.map_err(|_| ())?;

        let routing = atomically(|| {
            let routing = self.routing.read()?;

            let route = route.clone();
            let mut routes = routing.routes().clone();
            routes.insert(Arc::new(route.ref_()), route);

            let routing = SourceRoutingConfiguration::builder()
                .listener(routing.listener().clone())
                .routes(routes)
                .shared_filters(routing.shared_filters().clone())
                .build();

            self.routing.write(routing)?;

            self.routing.read()
        })
        .await;

        let _ = self.routing_tx.send(routing);

        Ok(())
    }

    async fn sync_shared_filter(&self, filter_ref: SharedFilterRef) -> Result<(), ()> {
        let shared_filter = self
            .api_client
            .shared_filter(&filter_ref)
            .await
            .map_err(|_| ())?;

        let routing = atomically(|| {
            let routing = self.routing.read()?;

            let mut shared_filters = routing.shared_filters().clone();
            shared_filters.insert(Arc::new(shared_filter.ref_()), shared_filter.clone());

            let routing = SourceRoutingConfiguration::builder()
                .listener(routing.listener.clone())
                .routes(routing.routes.clone())
                .shared_filters(shared_filters)
                .build();

            self.routing.write(routing)?;

            self.routing.read()
        })
        .await;

        let _ = self.routing_tx.send(routing);

        Ok(())
    }

    async fn sync_backend(&self, backend_ref: BackendRef) -> Result<(), ()> {
        let backend = self
            .api_client
            .backend(&backend_ref)
            .await
            .map_err(|_| ())?;

        let backends = atomically(|| {
            let backends = self.backends.read()?;
            let mut backends = backends.as_ref().clone();
            backends
                .backends
                .insert(Arc::new(backend.ref_()), backend.clone());
            self.backends.write(backends)?;

            self.backends.read()
        })
        .await;

        let _ = self.backends_tx.send(backends);
        Ok(())
    }
}
