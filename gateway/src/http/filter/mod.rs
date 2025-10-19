use std::collections::HashMap;
use std::sync::Arc;
use async_stm::{atomically, TVar};
use getset::{CloneGetters, Getters};
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::filter::{SharedFilterRef};
use vg_core::net::topology::TopologyLocation;
use vg_core::sync::arc_watch::{channel, Receiver, Sender};
use vg_core::sync::handles::{handles, Handle};
use vg_http::filter::SharedFilterHandler;
use crate::configuration::{SourceBackendConfiguration, SourceRoutingConfiguration};
use crate::http::backend::BackendConfiguration;

#[derive(TypedBuilder, Default, Clone, Debug, Getters, CloneGetters)]
pub struct SharedFilterHandlers {
    handlers: HashMap<Arc<SharedFilterRef>, Arc<SharedFilterHandler>>
}

#[derive(TypedBuilder)]
pub struct SharedFilterHandlersManagerOptions {
    source_routing_rx: Receiver<SourceRoutingConfiguration>
}

#[derive(TypedBuilder)]
pub struct SharedFilterHandlersManager {
    source_routing_rx: Receiver<SourceRoutingConfiguration>,
    handlers: TVar<SharedFilterHandlers>,
    handlers_tx: Sender<SharedFilterHandlers>,
    handlers_rx: Receiver<SharedFilterHandlers>
}

impl From<SharedFilterHandlersManagerOptions> for SharedFilterHandlersManager {
    fn from(value: SharedFilterHandlersManagerOptions) -> Self {
        let (tx, rx) = channel();
        
        Self::builder()
            .source_routing_rx(value.source_routing_rx)
            .handlers(Default::default())
            .handlers_tx(tx)
            .handlers_rx(rx)
            .build()
    }
}

impl SharedFilterHandlersManager {
    pub  fn handlers(&self) -> Receiver<SharedFilterHandlers> {
        self.handlers_rx.clone()
    }
    
    pub  fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut source_routing_rx = self.source_routing_rx;
        let handlers_t = self.handlers;
        let handlers_tx = self.handlers_tx;
        
        spawn(async move { 
            loop {
                let handlers = atomically(|| {
                    let source_routing = source_routing_rx.current().unwrap_or_default();
                    let handlers = source_routing.shared_filters().iter()
                        .filter_map(|(ref_, filter)| {
                            let handler: SharedFilterHandler =  filter.as_ref().try_into().ok()?;
                            Some((ref_.clone(), Arc::new(handler)))
                        })
                        .collect();
                    
                    let handlers = SharedFilterHandlers::builder()
                        .handlers(handlers)
                        .build();
                    
                    handlers_t.write(handlers)?;
                    
                    handlers_t.read()
                }).await;
                
                let _ = handlers_tx.send(handlers);

                select! {
                    _ = source_routing_rx.changed() => {
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