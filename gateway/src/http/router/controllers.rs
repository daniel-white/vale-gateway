use crate::http::filters::HttpFilterHandlers;
use crate::http::listener::filters::HttpListenerFilterHandler;
use crate::http::router::router::HttpRouter;
use crate::http::router::routes::HttpRoute;
use crate::infra::TopologyLocation;
use futures::{stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use vg_core::http::routes::{HttpRoute as CoreHttpRoute, HttpRouteKey};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

fn http_routes(
    task_builder: &TaskBuilder,
    http_listener_routes_rx: &Receiver<Vec<CoreHttpRoute>>,
    http_filter_handlers: &HttpFilterHandlers,
    current_location: Arc<TopologyLocation>,
) -> Receiver<HashMap<HttpRouteKey, Arc<HttpRoute>>> {
    let (tx, rx) = signal(stringify!(http_routes));
    let http_listener_routes_rx = http_listener_routes_rx.clone();
    let http_filter_handlers = http_filter_handlers.clone();
    let current_location = current_location.clone();

    task_builder
        .new_task(stringify!(http_routes))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(routes) = await_ready!(http_listener_routes_rx)
                    && http_filter_handlers.ready().await.is_ready()
                {
                    let routes = stream::iter(routes)
                        .then(|route| async {
                            let route = HttpRoute::from(
                                route,
                                &http_filter_handlers,
                                current_location.clone(),
                            )
                            .await;

                            (route.key().clone(), Arc::new(route))
                        })
                        .collect()
                        .await;

                    tx.set(routes).await;
                }

                continue_on!(
                    http_listener_routes_rx.changed(),
                    http_filter_handlers.changed()
                )
            }
        });

    rx
}

pub fn http_router(
    task_builder: &TaskBuilder,
    http_listener_routes_rx: &Receiver<Vec<CoreHttpRoute>>,
    http_listener_filter_handlers_rx: &Receiver<Vec<HttpListenerFilterHandler>>,
    http_filter_handlers: &HttpFilterHandlers,
    current_location: Arc<TopologyLocation>,
) -> Receiver<Arc<HttpRouter>> {
    let (tx, rx) = signal(stringify!(http_router));
    let http_listener_filter_handlers_rx = http_listener_filter_handlers_rx.clone();
    let http_routes_rx = http_routes(
        task_builder,
        http_listener_routes_rx,
        http_filter_handlers,
        current_location,
    );

    task_builder
        .new_task(stringify!(http_router))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((filters, routes)) =
                    await_ready!(http_listener_filter_handlers_rx, http_routes_rx)
                    && let ReadyState::Ready(routes) = await_ready!(http_routes_rx)
                {
                    let http_router = HttpRouter::builder()
                        .filters(filters.clone())
                        .routes(routes.clone())
                        .build();
                    tx.set(Arc::new(http_router)).await;
                }
                continue_on!(
                    http_listener_filter_handlers_rx.changed(),
                    http_routes_rx.changed()
                )
            }
        });

    rx
}
