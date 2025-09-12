use crate::http::filters::HttpFilterHandlers;
use crate::http::listener::filters::{
    HttpListenerFilterHandler, collect_http_listener_filter_handlers,
};
use std::sync::Arc;
use vg_core::gateways::Gateway;
use vg_core::http::listeners::{HttpListener, HttpListenerFilter};
use vg_core::http::routes::HttpRoute;
use vg_core::sync::signal::{Receiver, signal};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{ReadyState, await_ready, continue_on};

pub fn http_listener(
    task_builder: &TaskBuilder,
    gateway_rx: &Receiver<Arc<Gateway>>,
) -> Receiver<Option<HttpListener>> {
    let (tx, rx) = signal(stringify!(http_listener));
    let gateway_rx = gateway_rx.clone();

    task_builder
        .new_task(stringify!(http_listener))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(gateway) = await_ready!(gateway_rx) {
                    let http_listener = gateway.http_listener().as_ref().map(|arc| (**arc).clone());
                    tx.set(http_listener).await;
                }
                continue_on!(gateway_rx.changed());
            }
        });

    rx
}

pub fn http_listener_routes(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<Vec<HttpRoute>> {
    let (tx, rx) = signal(stringify!(http_listener_routes));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(http_listener_routes))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let filters = http_listener
                        .as_ref()
                        .map(|listener| listener.routes().clone())
                        .unwrap_or_default();
                    tx.set(filters).await;
                }
                continue_on!(http_listener_rx.changed());
            }
        });

    rx
}

fn http_listener_filters(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<Vec<HttpListenerFilter>> {
    let (tx, rx) = signal(stringify!(http_listener_filters));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(http_listener_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let filters = http_listener
                        .as_ref()
                        .map(|listener| listener.filters().clone())
                        .unwrap_or_default();
                    tx.set(filters).await;
                }
                continue_on!(http_listener_rx.changed());
            }
        });

    rx
}

pub fn http_listener_filter_handlers(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
    http_filter_handlers: &HttpFilterHandlers,
) -> Receiver<Vec<HttpListenerFilterHandler>> {
    let (tx, rx) = signal(stringify!(http_listener_filter_handlers));
    let http_listener_filters_rx = http_listener_filters(task_builder, http_listener_rx);
    let http_filter_handlers = http_filter_handlers.clone();

    task_builder
        .new_task(stringify!(http_listener_filter_handlers))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener_filters) =
                    await_ready!(http_listener_filters_rx)
                    && http_filter_handlers.ready().await.is_ready()
                {
                    let handlers = collect_http_listener_filter_handlers(
                        &http_filter_handlers,
                        http_listener_filters,
                    )
                    .await;

                    tx.set(handlers).await;
                }
                continue_on!(
                    http_listener_filters_rx.changed(),
                    http_filter_handlers.changed()
                );
            }
        });

    rx
}
