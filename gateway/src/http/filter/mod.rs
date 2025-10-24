use crate::configuration::RoutingConfiguration;
use getset::{CloneGetters, Getters};
use std::collections::HashMap;
use std::ops::Sub;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_core::sync::arc_watch::{Receiver, Sender, channel};
use vg_core::sync::handles::{Handle, handles};
use vg_core::sync::observable::Subscription;
use vg_http::filter::SharedFilterHandler;

#[derive(TypedBuilder, Default, Debug, Getters, CloneGetters)]
pub struct SharedFilterHandlers {
    #[getset(get = "pub")]
    handlers: HashMap<SharedFilterRef, SharedFilterHandler>,
}

#[derive(TypedBuilder)]
pub struct SharedFilterHandlersManagerOptions {
    routing: Subscription<RoutingConfiguration>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct SharedFilterHandlersManager {
    routing: Subscription<RoutingConfiguration>,
    handlers: Sender<SharedFilterHandlers>,
}

impl From<SharedFilterHandlersManagerOptions> for SharedFilterHandlersManager {
    fn from(value: SharedFilterHandlersManagerOptions) -> Self {
        let (handlers, _) = channel();

        Self::builder()
            .routing(value.routing)
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
            let mut routing = self.routing;

            loop {
                let handlers = {
                    let handlers = routing.current()
                        .shared_filters()
                        .iter()
                        .filter_map(|(ref_, filter)| {
                            let handler: SharedFilterHandler = filter.as_ref().try_into().ok()?;
                            Some((ref_.clone(), handler))
                        })
                        .collect();

                    SharedFilterHandlers::builder().handlers(handlers).build()
                };

                let _ = self.handlers.send(Arc::new(handlers));

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
