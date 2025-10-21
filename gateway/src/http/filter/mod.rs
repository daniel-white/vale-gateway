use crate::configuration::SourceRoutingConfiguration;
use async_stm::{TVar, atomically};
use getset::{CloneGetters, Getters};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_core::sync::arc_watch::{Receiver, Sender, channel};
use vg_core::sync::handles::{Handle, handles};
use vg_http::filter::SharedFilterHandler;

#[derive(TypedBuilder, Default, Clone, Debug, Getters, CloneGetters)]
pub struct SharedFilterHandlers {
    #[allow(dead_code)]
    handlers: HashMap<Arc<SharedFilterRef>, Arc<SharedFilterHandler>>,
}

#[derive(TypedBuilder)]
pub struct SharedFilterHandlersManagerOptions {
    routing: Receiver<SourceRoutingConfiguration>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct SharedFilterHandlersManager {
    routing: Receiver<SourceRoutingConfiguration>,
    handlers: TVar<SharedFilterHandlers>,
    handlers_tx: Sender<SharedFilterHandlers>,
}

impl From<SharedFilterHandlersManagerOptions> for SharedFilterHandlersManager {
    fn from(value: SharedFilterHandlersManagerOptions) -> Self {
        let (tx, _) = channel();

        Self::builder()
            .routing(value.routing)
            .handlers(Default::default())
            .handlers_tx(tx)
            .build()
    }
}

impl SharedFilterHandlersManager {
    #[allow(dead_code)]
    pub fn handlers(&self) -> Receiver<SharedFilterHandlers> {
        self.handlers_tx.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();
        let mut routing = self.routing;
        let handlers_t = self.handlers;
        let handlers_tx = self.handlers_tx;

        spawn(async move {
            loop {
                let handlers = atomically(|| {
                    let routing = routing.current().unwrap_or_default();
                    let handlers = routing
                        .shared_filters()
                        .iter()
                        .filter_map(|(ref_, filter)| {
                            let handler: SharedFilterHandler = filter.as_ref().try_into().ok()?;
                            Some((ref_.clone(), Arc::new(handler)))
                        })
                        .collect();

                    let handlers = SharedFilterHandlers::builder().handlers(handlers).build();

                    handlers_t.write(handlers)?;

                    handlers_t.read()
                })
                .await;

                let _ = handlers_tx.send(handlers);

                select! {
                    _ = routing.changed() => {
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
