use crate::http::routes::backends::collect_http_backends;
use crate::kubernetes::objects::ObjectRef;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use gateway_api::gateways::Gateway;
use gateway_api::httproutes::HTTPRoute;
use getset::{CloneGetters, Getters, Setters};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::info;
use typed_builder::TypedBuilder;
use vg_core::http::listeners::HttpBackend;
use vg_core::http::routes::backends::HttpRouteBackend;
use vg_core::http::routes::rules::HttpRouteRuleKey;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

#[derive(Debug, Getters, CloneGetters, TypedBuilder, PartialEq, Clone)]
pub struct HttpRouteInfo {
    #[getset(get_clone = "pub")]
    http_route: Arc<HTTPRoute>,

    #[getset(get_clone = "pub")]
    backends: Arc<HashMap<HttpRouteRuleKey, Vec<HttpRouteBackend>>>,
}

pub fn http_routes(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client_rx: &Receiver<KubeClientCell>,
) -> (
    Receiver<HashMap<ObjectRef, Vec<HttpRouteInfo>>>,
    Receiver<HashSet<ObjectRef>>,
) {
    let (http_routes_tx, http_routes_rx) = signal(stringify!(http_routes));
    let (http_backend_refs_tx, http_backend_refs_rx) = signal(stringify!(http_backends));
    let source_http_routes_rx = watch_objects!(options, task_builder, HTTPRoute, client_rx);

    task_builder
        .new_task(stringify!(http_routes))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_routes) = await_ready!(source_http_routes_rx) {
                    info!("Collecting HTTPRoutes by Gateway");
                    let mut routes = HashMap::new();
                    let mut backend_refs = HashSet::new();

                    for (http_route_ref, _, http_route) in http_routes.iter() {
                        info!("Collecting HTTPRoute: object.ref={}", http_route_ref);

                        for parent_ref in http_route.spec.parent_refs.iter().flatten() {
                            let gateway_ref = ObjectRef::of_kind::<Gateway>()
                                .namespace(
                                    parent_ref
                                        .namespace
                                        .as_ref()
                                        .or_else(|| http_route_ref.namespace().as_ref())
                                        .cloned(),
                                )
                                .name(&parent_ref.name)
                                .build();

                            let routes = routes.entry(gateway_ref).or_insert_with(Vec::new);

                            let route_info = HttpRouteInfo::builder()
                                .http_route(http_route.clone())
                                .backends(Arc::new(collect_http_backends(
                                    &http_route,
                                    &mut backend_refs,
                                )))
                                .build();

                            routes.push(route_info);
                        }
                    }

                    http_routes_tx.set(routes).await;
                    http_backend_refs_tx.set(backend_refs).await;
                }

                continue_on!(source_http_routes_rx.changed());
            }
        });

    (http_routes_rx, http_backend_refs_rx)
}
