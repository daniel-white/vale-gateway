use std::sync::Arc;
use dashmap::DashMap;
use tokio::{select, spawn};
use tokio::sync::RwLock;
use typed_builder::TypedBuilder;
use vg_config::http::backend::{Backend, BackendRef};
use vg_config::http::listener::Listener;
use vg_config::http::route::{Route, RouteRef};
use vg_core::sync::handles::{handles, Handle};
use vg_rpc_client::{ConfigurationClient, ConfigurationEventReceiver};

#[derive(TypedBuilder)]
pub struct SourceConfigurationRegistry {
    #[builder(default, setter(skip))]
    listener: Arc<RwLock<Option<Arc<Listener>>>>,
    #[builder(default, setter(skip))]
    routes: DashMap<RouteRef, Arc<Route>>,
    #[builder(default, setter(skip))]
    backends: DashMap<BackendRef, Arc<Backend>>,
    client: ConfigurationClient,
    event_rx: ConfigurationEventReceiver
}

impl SourceConfigurationRegistry {
    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let processor = ConfigurationEventProcessor::builder()
            .client(self.client)
            .build();
        
        spawn(async move {
            let mut event_rx = self.event_rx;
            println!("closed? {}", event_rx.is_closed());
           loop {
               select! {
                   event = event_rx.recv() => {
                       println!("event! {:?}", event);
                   },
                   _ = stop_handle.stopped() => {
                       break;
                   }
               }
           } 
        });
        
        handle
    }
}

#[derive(TypedBuilder)]
struct ConfigurationEventProcessor {
    client: ConfigurationClient,
}
