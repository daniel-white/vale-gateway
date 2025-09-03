use crate::gateways::collectors::Gateways;
use crate::http::filters::collector::{collect_http_filters, HttpFilters};
use crate::http::filters::extensions::HttpExtensionFilters;
use crate::http::routes::controllers::{http_routes, HttpRouteInfo};
use crate::http::routes::converters::convert_http_route;
use crate::kubernetes::objects::ObjectRef;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use futures::{stream, StreamExt};
use gateway_api::gateways::{GatewayListeners, GatewayListenersAllowedRoutesNamespacesFrom};
use gateway_api::httproutes::HTTPRoute;
use std::collections::HashMap;
use std::num::NonZeroU16;
use std::sync::Arc;
use vg_core::http::listeners::HttpListener;
use vg_core::net::Port;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_listeners(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client: &Receiver<KubeClientCell>,
    gateways_rx: &Receiver<Arc<Gateways>>,
) -> Receiver<HashMap<ObjectRef, HttpListener>> {
    let (tx, rx) = signal(stringify!(http_listeners));
    let gateways_rx = gateways_rx.clone();
    let (http_routes_rx, http_backend_refs_rx) = http_routes(task_builder, options.clone(), client);
    let http_extension_filters_rx =
        HttpExtensionFilters::new(task_builder, options.clone(), client);

    task_builder
        .new_task(stringify!(http_listeners))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((gateways, http_routes, http_backend_refs)) =
                    await_ready!(gateways_rx, http_routes_rx, http_backend_refs_rx)
                    && let ReadyState::Ready(http_extension_filters) =
                        http_extension_filters_rx.ready().await
                {
                    let http_listeners =
                        collect_http_listeners(gateways, http_routes, http_extension_filters).await;

                    tx.set(http_listeners).await;
                }

                continue_on!(
                    gateways_rx.changed(),
                    http_routes_rx.changed(),
                    http_backend_refs_rx.changed(),
                    http_extension_filters_rx.changed()
                );
            }
        });

    rx
}

async fn collect_http_listeners(
    gateways: &Arc<Gateways>,
    http_routes: &HashMap<ObjectRef, Vec<HttpRouteInfo>>,
    http_extension_filters: &HttpExtensionFilters,
) -> HashMap<ObjectRef, HttpListener> {
    stream::iter(gateways.iter())
        .filter_map(
            |(gateway_ref, class, class_parameters, gateway, gateway_parameters)| async {
                if let Some(http_listener) = gateway
                    .spec
                    .listeners
                    .iter()
                    .find(|listener| listener.protocol == "HTTP")
                    .cloned()
                {
                    Some((
                        gateway_ref.clone(),
                        class,
                        class_parameters,
                        gateway,
                        gateway_parameters,
                        http_listener,
                    ))
                } else {
                    None
                }
            },
        )
        .then(
            |(
                gateway_ref,
                _class,
                _class_parameters,
                _gateway,
                gateway_parameters,
                http_listener,
            )| async move {
                let routes = http_routes.get(&gateway_ref).cloned().unwrap_or_default();
                let http_listener_parameters = gateway_parameters
                    .as_ref()
                    .spec
                    .common
                    .as_ref()
                    .and_then(|params| params.gateway.as_ref())
                    .and_then(|gateways| gateways.listeners.as_ref())
                    .and_then(|listeners| listeners.http.as_ref())
                    .cloned()
                    .unwrap_or_default();
                let filters = collect_http_filters(
                    &http_listener_parameters,
                    &routes,
                    http_extension_filters,
                )
                .await;

                (gateway_ref, http_listener, routes, filters)
            },
        )
        .then(|(gateway_ref, http_listener, routes, filters)| async move {
            let http_listener =
                convert_http_listener(&gateway_ref, &http_listener, &routes, &filters);
            (gateway_ref, http_listener)
        })
        .collect()
        .await
}

fn convert_http_listener(
    gateway_ref: &ObjectRef,
    http_listener: &GatewayListeners,
    routes: &Vec<HttpRouteInfo>,
    http_filters: &HttpFilters,
) -> HttpListener {
    let mut builder = HttpListener::builder();

    {
        let port: Port = if http_listener.port > 0 {
            NonZeroU16::new(http_listener.port as u16).unwrap().into()
        } else {
            NonZeroU16::new(80).unwrap().into()
        };

        builder.port(port);
    }

    {
        let routes = if let Some(allowed_routes) = &http_listener.allowed_routes
            && let Some(namespaces) = &allowed_routes.namespaces
            && let Some(from) = &namespaces.from
        {
            match (from, namespaces.selector.as_ref()) {
                (GatewayListenersAllowedRoutesNamespacesFrom::All, _) => routes.clone(),
                (GatewayListenersAllowedRoutesNamespacesFrom::Same, _) => routes
                    .iter()
                    .filter(|route| {
                        route.http_route().metadata.namespace.as_ref()
                            == gateway_ref.namespace().as_ref()
                    })
                    .cloned()
                    .collect(),
                (GatewayListenersAllowedRoutesNamespacesFrom::Selector, Some(selector)) => {
                    // TODO: Optimize by caching label selectors
                    Vec::new()
                }
                (GatewayListenersAllowedRoutesNamespacesFrom::Selector, None) => Vec::new(),
            }
        } else {
            routes.clone()
        };

        for route in routes {
            let http_route = route.http_route();
            let key = format!(
                "{}-{}",
                http_route.metadata.namespace.as_deref().unwrap_or(""),
                http_route.metadata.name.as_deref().unwrap_or(""),
            );
            builder.add_route(&key, |builder| {
                convert_http_route(builder, &key, &route, &http_filters);
            });
        }
    }

    for filter in http_filters.listener_filters() {
        builder.add_filter(filter.clone());
    }

    for (_, filter_definition) in http_filters.filter_definitions() {
        builder.add_filter_definition(filter_definition.clone());
    }

    builder.build()
}
