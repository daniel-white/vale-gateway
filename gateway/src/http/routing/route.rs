use std::collections::HashMap;
use std::sync::Arc;
use getset::{CloneGetters, Getters};
use tokio::{select, spawn};
use typed_builder::TypedBuilder;
use vg_config::http::route::RouteRef;
use vg_core::sync::arc_watch::{channel, Receiver, Sender};
use vg_core::sync::handles::{handles, Handle};
use vg_http::route::host::HostMatcher;
use crate::configuration::RoutingConfiguration;
use crate::http::filter::SharedFilterHandlers;

#[derive(TypedBuilder, Debug, Getters, CloneGetters)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct Route {
    #[getset(get_clone = "pub")]
    ref_: RouteRef,
    
    host_matchers: Vec<HostMatcher>,
}

#[derive(Debug, Default, Clone, TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct Routes {
    routes: HashMap<RouteRef, Arc<Route>>
}

#[derive(TypedBuilder)]
pub struct RouteConfiguratorOptions {
    routing_configuration: Receiver<RoutingConfiguration>,
    shared_filter_handlers: Receiver<SharedFilterHandlers>
}

#[derive(TypedBuilder)]
#[builder(builder_method(vis = ""), builder_type(vis = ""))]
pub struct RouteConfigurator {
    routing_configuration: Receiver<RoutingConfiguration>,
    shared_filter_handlers: Receiver<SharedFilterHandlers>,
    routes: Sender<Routes>,
}

impl From<RouteConfiguratorOptions> for RouteConfigurator {
    fn from(value: RouteConfiguratorOptions) -> Self {
        let (routes, _) = channel();

        Self::builder()
            .routing_configuration(value.routing_configuration)
            .shared_filter_handlers(value.shared_filter_handlers)
            .routes(routes)
            .build()
    }
}

impl RouteConfigurator {
    pub fn routes(&self) -> Receiver<Routes> {
        self.routes.subscribe()
    }
    
    pub  fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            let mut routing = self.routing_configuration;
            let mut shared_filter_handlers = self.shared_filter_handlers;
            loop{
                let routes: Vec<_> = {
                    let routing = routing.current().unwrap_or_default();
                    let shared_filter_handlers = shared_filter_handlers.current().unwrap_or_default();
                    
                    routing.routes().values().map(|r| {
                        Route::builder()
                            .ref_(r.ref_())
                            .host_matchers(Vec::new())
                            .build()
                    }).collect()
                };
                
                let _ = self.routes.send(Arc::new(Routes::default()));
                
                select! {
                    _ = routing.changed() => {
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