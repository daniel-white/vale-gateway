use crate::configuration::RoutingConfiguration;
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
    handlers: HashMap<SharedFilterRef, Arc<SharedFilterHandler>>,
}

#[derive(TypedBuilder)]
pub struct SharedFilterHandlersManagerOptions {
    routing_configuration: Receiver<RoutingConfiguration>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct SharedFilterHandlersManager {
    routing_configuration: Receiver<RoutingConfiguration>,
    handlers: Sender<SharedFilterHandlers>,
}

impl From<SharedFilterHandlersManagerOptions> for SharedFilterHandlersManager {
    fn from(value: SharedFilterHandlersManagerOptions) -> Self {
        let (handlers, _) = channel();

        Self::builder()
            .routing_configuration(value.routing_configuration)
            .handlers(handlers)
            .build()
    }
}

impl SharedFilterHandlersManager {
    pub fn handlers(&self) -> Receiver<SharedFilterHandlers> {
        self.handlers.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            let mut routing = self.routing_configuration;
            let handlers_t = TVar::new(Default::default());
            
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

                    let handlers = SharedFilterHandlers::builder().handlers(Arc::new(handlers)).build();

                    handlers_t.write(handlers)?;

                    handlers_t.read()
                })
                .await;

                let _ = self.handlers.send(handlers);

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
