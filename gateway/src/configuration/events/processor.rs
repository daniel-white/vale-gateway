use crate::configuration::{SourceBackendConfiguration, SourceRoutingConfiguration};
use async_stm::{TVar, atomically};
use futures::future::join_all;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::filter::{SharedFilter, SharedFilterRef};
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::arc_watch::Sender;
use vg_rpc_client::{
    ConfigurationClient, ConfigurationClientError, ConfigurationEvent, ErrorClassification,
};

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
    pub async fn init(&self) -> Result<(), ()> {
        self.sync_all().await
    }

    pub async fn handle(&self, event: ConfigurationEvent) -> Result<(), ()> {
        match event {
            ConfigurationEvent::ListenerChanged => self.sync_all().await,
            ConfigurationEvent::RouteChanged(route_ref) => self.sync_route(route_ref).await,
            ConfigurationEvent::BackendChanged(backend_ref) => self.sync_backend(backend_ref).await,
            ConfigurationEvent::SharedFilterChanged(filter_ref) => {
                self.sync_shared_filter(filter_ref).await
            }
        }
    }

    async fn sync_all(&self) -> Result<(), ()> {
        let listener = match self.client.listener().await {
            Ok(listener) => listener,
            Err(e) => {
                // Use existing error classification to handle client errors gracefully
                if e.is_temporary() {
                    tracing::debug!("Temporary error fetching listener, will retry: {}", e);
                    return Err(());
                } else {
                    tracing::error!("Unrecoverable error fetching listener: {}", e);
                    return Err(());
                }
            }
        };

        let routes = self.fetch_routes(listener.route_refs()).await;
        let shared_filters = self
            .fetch_shared_filters(listener.shared_filter_refs())
            .await;
        let backends = self.fetch_backends(listener.backend_refs()).await;

        let (routing, backends) = atomically(|| {
            let routes = routes
                .iter()
                .filter_map(|r| r.clone().ok())
                .map(|route| (Arc::new(route.ref_()), route))
                .collect();
            let shared_filters = shared_filters
                .iter()
                .filter_map(|r| r.clone().ok())
                .map(|filter| (Arc::new(filter.ref_()), filter))
                .collect();

            let routing = SourceRoutingConfiguration::builder()
                .listener(Some(listener.clone()))
                .routes(routes)
                .shared_filters(shared_filters)
                .build();

            self.routing.write(routing)?;

            let backends = backends
                .iter()
                .filter_map(|r| r.clone().ok())
                .map(|backend| (Arc::new(backend.ref_()), backend))
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
        let route = match self.client.route(&route_ref).await {
            Ok(route) => route,
            Err(e) => {
                // Use existing error classification to handle client errors gracefully
                if e.is_temporary() {
                    tracing::debug!(
                        "Temporary error fetching route {:?}, will retry: {}",
                        route_ref,
                        e
                    );
                    return Err(());
                } else {
                    // Log at appropriate level based on error type
                    match &e {
                        ConfigurationClientError::NotFound => {
                            tracing::info!("Route {:?} not found: {}", route_ref, e);
                        }
                        _ => {
                            tracing::error!(
                                "Unrecoverable error fetching route {:?}: {}",
                                route_ref,
                                e
                            );
                        }
                    }
                    return Err(());
                }
            }
        };

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
        let shared_filter = match self.client.shared_filter(&filter_ref).await {
            Ok(filter) => filter,
            Err(e) => {
                // Use existing error classification to handle client errors gracefully
                if e.is_temporary() {
                    tracing::debug!(
                        "Temporary error fetching shared filter {:?}, will retry: {}",
                        filter_ref,
                        e
                    );
                    return Err(());
                } else {
                    tracing::error!(
                        "Unrecoverable error fetching shared filter {:?}: {}",
                        filter_ref,
                        e
                    );
                    return Err(());
                }
            }
        };

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
        let backend = match self.client.backend(&backend_ref).await {
            Ok(backend) => backend,
            Err(e) => {
                // Use existing error classification to handle client errors gracefully
                if e.is_temporary() {
                    tracing::debug!(
                        "Temporary error fetching backend {:?}, will retry: {}",
                        backend_ref,
                        e
                    );
                    return Err(());
                } else {
                    tracing::error!(
                        "Unrecoverable error fetching backend {:?}: {}",
                        backend_ref,
                        e
                    );
                    return Err(());
                }
            }
        };

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

    async fn fetch_routes(
        &self,
        route_refs: &[RouteRef],
    ) -> Vec<Result<Arc<Route>, ConfigurationClientError>> {
        let routes = route_refs
            .iter()
            .map(|route_ref| self.client.route(route_ref));

        join_all(routes).await
    }

    async fn fetch_backends(
        &self,
        backend_refs: &[BackendRef],
    ) -> Vec<Result<Arc<Backend>, ConfigurationClientError>> {
        let backends = backend_refs
            .iter()
            .map(|backend_ref| self.client.backend(backend_ref));

        join_all(backends).await
    }

    async fn fetch_shared_filters(
        &self,
        filter_refs: &[SharedFilterRef],
    ) -> Vec<Result<Arc<SharedFilter>, ConfigurationClientError>> {
        let filters = filter_refs
            .iter()
            .map(|filter_ref| self.client.shared_filter(filter_ref));

        join_all(filters).await
    }
}
