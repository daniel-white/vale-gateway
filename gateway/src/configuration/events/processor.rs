use vg_core::sync::arc_watch::Sender;
use crate::configuration::{SourceBackendConfiguration, SourceRoutingConfiguration};
use async_stm::{TVar, atomically};
use futures::future;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::route::{Route, RouteRef};
use vg_rpc_client::{ConfigurationClient, ConfigurationClientError, ConfigurationEvent};

#[derive(TypedBuilder)]
pub struct ConfigurationEventProcessor {
    client: ConfigurationClient,
    #[builder(default, setter(skip))]
    backends: TVar<SourceBackendConfiguration>,
    backends_tx: Sender<SourceBackendConfiguration>,
    #[builder(default, setter(skip))]
    routing: TVar<SourceRoutingConfiguration>,
    routing_tx: Sender<SourceRoutingConfiguration>,
}

impl ConfigurationEventProcessor {
    pub async fn init(&self) {
        let _ = self.sync_all().await;
    }

    pub async fn handle(&self, event: ConfigurationEvent) {
        match event {
            ConfigurationEvent::ListenerChanged => {
                let _ = self.sync_all().await;
            }
            ConfigurationEvent::RouteChanged(route_ref) => {
                let _ = self.sync_route(route_ref).await;
            }
            ConfigurationEvent::BackendChanged(backend_ref) => {
                let _ = self.sync_backend(backend_ref).await;
            }
        };
    }

    async fn sync_all(&self) -> Result<(), ()> {
        let listener = self.client.listener().await.map_err(|_| ())?;
        let routes = self.fetch_routes(listener.route_refs()).await;
        let backends = self.fetch_backends(listener.backend_refs()).await;

        let (routing, backends) = atomically(|| {
            let routes = routes
                .clone()
                .into_iter()
                .filter_map(|r| r.ok())
                .map(|r| (r.ref_().clone(), r))
                .collect();

            let routing = SourceRoutingConfiguration::builder()
                .listener(Some(listener.clone()))
                .routes(routes)
                .build();

            self.routing.write(routing)?;

            let backends = backends
                .clone()
                .into_iter()
                .filter_map(|b| b.ok())
                .map(|b| (b.ref_().clone(), b))
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
        let route = self.client.route(&route_ref).await.map_err(|_| ())?;

        let routing = atomically(|| {
            let routing = self.routing.read()?;


                let listener = routing.listener().clone();
                let route_ref = route_ref.clone();
                let route = route.clone();
                let mut routes = routing.routes().clone();
                routes.insert(route_ref, route);

                let routing = SourceRoutingConfiguration::builder()
                    .listener(listener)
                    .routes(routes)
                    .build();

                self.routing.write(routing)?;

            self.routing.read()
        })
        .await;


            let _ = self.routing_tx.send(routing);

        Ok(())
    }

    async fn sync_backend(&self, backend_ref: BackendRef) -> Result<(), ()> {
        let backend = self.client.backend(&backend_ref).await.map_err(|_| ())?;

        let backends = atomically(|| {
            let backends = self.backends.read()?;
            let mut backends = backends.as_ref().clone();
            backends
                .backends
                .insert(backend_ref.clone(), backend.clone());
            self.backends.write(backends)?;

            self.backends.read()
        })
        .await;

        let _ = self.backends_tx.send(backends);
        Ok(())
    }

    async fn fetch_routes(
        &self,
        route_refs: &[RouteRef],
    ) -> Vec<Result<Route, ConfigurationClientError>> {
        let routes = route_refs
            .iter()
            .map(|route_ref| self.client.route(route_ref));

        future::join_all(routes).await
    }

    async fn fetch_backends(
        &self,
        backend_refs: &[BackendRef],
    ) -> Vec<Result<Backend, ConfigurationClientError>> {
        let backends = backend_refs
            .iter()
            .map(|backend_ref| self.client.backend(backend_ref));

        future::join_all(backends).await
    }
}
