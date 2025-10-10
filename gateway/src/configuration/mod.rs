use dashmap::DashMap;
use getset::CloneGetters;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender, channel};
use tokio::task::JoinHandle;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_rpc_client::{
    ConfigurationClient, ConfigurationEvent,
};

#[derive(Clone)]
pub struct ConfigurationRegistry {
    listener: Arc<RwLock<Option<Arc<Listener>>>>,
    routes: Arc<DashMap<Arc<RouteRef>, Arc<Route>>>,
    backends: Arc<DashMap<Arc<BackendRef>, Arc<Backend>>>,
    tx: Sender<ConfigurationRegistryEvent>,
}

#[derive(Clone, Debug)]
pub enum ConfigurationRegistryEvent {
    HttpConfigurationChanged(HttpConfigurationSnapshot),
    BackendConfigurationChanged(BackendConfigurationSnapshot),
}

#[derive(Debug, Clone, CloneGetters, TypedBuilder)]
pub struct HttpConfigurationSnapshot {
    #[getset(get_clone = "pub")]
    listener: Arc<Listener>,

    #[getset(get_clone = "pub")]
    routes: Arc<HashMap<Arc<RouteRef>, Arc<Route>>>,
}

#[derive(Debug, Clone, CloneGetters, TypedBuilder)]
pub struct BackendConfigurationSnapshot {
    #[getset(get_clone = "pub")]
    backends: Arc<HashMap<Arc<BackendRef>, Arc<Backend>>>,
}

#[derive(Debug, Error)]
pub enum ConfigurationRegistryRecvEventError {
    #[error("Channel is closed")]
    Closed,
    #[error("Channel has lagged")]
    Lagged,
}

#[derive(TypedBuilder)]
pub struct ConfigurationRegistryEventReceiver {
    rx: Receiver<ConfigurationRegistryEvent>,
}

impl ConfigurationRegistryEventReceiver {
    pub async fn recv(
        &mut self,
    ) -> Result<ConfigurationRegistryEvent, ConfigurationRegistryRecvEventError> {
        match self.rx.recv().await {
            Ok(event) => Ok(event),
            Err(RecvError::Closed) => Err(ConfigurationRegistryRecvEventError::Closed),
            Err(RecvError::Lagged(_)) => Err(ConfigurationRegistryRecvEventError::Lagged),
        }
    }
}

impl ConfigurationRegistry {
    pub fn new() -> Self {
        let (tx, _) = channel(1024);
        Self {
            listener: Default::default(),
            routes: Default::default(),
            backends: Default::default(),
            tx,
        }
    }

    pub fn subscribe(&self) -> ConfigurationRegistryEventReceiver {
        let rx = self.tx.subscribe();
        ConfigurationRegistryEventReceiver::builder().rx(rx).build()
    }

    fn send_event(&mut self, event: ConfigurationRegistryEvent) {
        if self.tx.receiver_count() > 0 {
            let _ = self.tx.send(event);
        }
    }

    async fn send_http_configuration_changed(&mut self) {
        let listener = self.listener.read().await.clone();
        if let Some(listener) = listener {
            let routes: HashMap<_, _> = self
                .routes
                .iter()
                .map(|e| (e.key().clone(), e.value().clone()))
                .collect();
            let snapshot = HttpConfigurationSnapshot::builder()
                .listener(listener)
                .routes(Arc::new(routes))
                .build();
            let event = ConfigurationRegistryEvent::HttpConfigurationChanged(snapshot);
            self.send_event(event);
        }
    }

    async fn send_backend_configuration_changed(&mut self) {
        let backends: HashMap<_, _> = self
            .backends
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();
        let snapshot = BackendConfigurationSnapshot::builder()
            .backends(Arc::new(backends))
            .build();
        let event = ConfigurationRegistryEvent::BackendConfigurationChanged(snapshot);
        self.send_event(event);
    }

    async fn set_listener(&mut self, listener: Listener) {
        let _ = self.listener.write().await.insert(Arc::new(listener));
        self.send_http_configuration_changed().await
    }

    async fn set_route(&mut self, route_ref: RouteRef, route: Route) {
        let _ = self.routes.insert(Arc::new(route_ref), Arc::new(route));
        self.send_http_configuration_changed().await;
    }

    async fn remove_route(&mut self, route_ref: RouteRef) {
        let _ = self.routes.remove(&route_ref);
        self.send_http_configuration_changed().await;
    }

    async fn set_backend(&mut self, backend_ref: BackendRef, backend: Backend) {
        let _ = self
            .backends
            .insert(Arc::new(backend_ref), Arc::new(backend));
        self.send_backend_configuration_changed().await;
    }

    async fn remove_backend(&mut self, backend_ref: BackendRef) {
        let _ = self.backends.remove(&backend_ref);
        self.send_backend_configuration_changed().await;
    }
}

#[derive(TypedBuilder)]
pub struct ConfigurationRegistrySynchronizer {
    registry: ConfigurationRegistry,
    client: ConfigurationClient,
}

impl ConfigurationRegistrySynchronizer {
    pub fn start(&self) -> JoinHandle<()> {
        let registry = self.registry.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            let mut registry = registry;
            Self::seed(&client, &mut registry).await;

            let mut rx = client.event_receiver();

            loop {
                match rx.recv().await {
                    Ok(ConfigurationEvent::ListenerChanged) => {
                        println!("evt listener");
                        Self::seed(&client, &mut registry).await;
                    }
                    any => {
                        println!("unexpected: {:?}", any)
                    }
                }
            }
        })
    }

    async fn seed(client: &ConfigurationClient, registry: &mut ConfigurationRegistry) {
        if let Ok(listener) = client.listener().await { registry.set_listener(listener).await }
    }
}
