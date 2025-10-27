use crate::configuration::GatewayConfiguration;
use crate::http::filter::SharedFilterHandlers;
use getset::{CloneGetters, Getters};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::route::RouteRef;
use vg_core::sync::arc_watch::{Receiver, Sender, channel};
use vg_core::sync::handles::{Handle, handles};
use vg_core::sync::observable::Subscription;
use vg_http::route::Route;

#[derive(Debug, Default, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct Routes {
    routes: HashMap<RouteRef, Route>,
}

#[derive(TypedBuilder)]
pub struct RouteConfiguratorOptions {
    gateway: Subscription<GatewayConfiguration>,
    shared_filter_handlers: Receiver<SharedFilterHandlers>,
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct RouteConfigurator {
    gateway: Subscription<GatewayConfiguration>,
    shared_filter_handlers: Receiver<SharedFilterHandlers>,
    routes: Sender<Routes>,
}

impl From<RouteConfiguratorOptions> for RouteConfigurator {
    fn from(value: RouteConfiguratorOptions) -> Self {
        let (routes, _) = channel();

        Self::builder()
            .gateway(value.gateway)
            .shared_filter_handlers(value.shared_filter_handlers)
            .routes(routes)
            .build()
    }
}

impl RouteConfigurator {
    pub fn routes(&self) -> Receiver<Routes> {
        self.routes.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            let mut gateway = self.gateway;
            let mut shared_filter_handlers = self.shared_filter_handlers;
            loop {
                let routes: HashMap<_, _> = {
                    let config = gateway.current();
                    let shared_filter_handlers =
                        shared_filter_handlers.current().unwrap_or_default();
                    let shared_filter_handlers = shared_filter_handlers.handlers();

                    config
                        .routes()
                        .values()
                        .filter_map(|route| {
                            let route = Route::try_from((shared_filter_handlers, route.as_ref()));
                            match route {
                                Ok(route) => Some((route.ref_(), route)),
                                Err(_) => None, // TODO handle error
                            }
                        })
                        .collect()
                };

                let routes = Routes::builder().routes(routes).build();

                let _ = self.routes.send(Arc::new(routes));

                select! {
                    _ = gateway.changed() => {
                        continue;
                    }
                    _ = shared_filter_handlers.changed() => {
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
