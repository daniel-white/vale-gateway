use super::cache::{HttpStaticResponseFilterBodyCache, HttpStaticResponseFilterBodyCacheClient};
use super::handler::{HttpStaticResponseFilterHandler, HttpStaticResponseFilterHandlerBody};
use crate::infra::InstanceContext;
use reqwest_middleware::ClientWithMiddleware;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use vg_core::http::filters::static_response::{
    HttpStaticResponseFilter, HttpStaticResponseFilterKey,
};
use vg_core::http::listeners::HttpFilterDefinition;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

fn http_static_response_filters(
    task_builder: &TaskBuilder,
    http_filter_definitions_rx: &Receiver<Vec<HttpFilterDefinition>>,
) -> Receiver<HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>> {
    let (tx, rx) = signal(stringify!(http_static_response_filters));
    let http_filter_definitions_rx = http_filter_definitions_rx.clone();

    task_builder
        .new_task(stringify!(http_static_response_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(http_filter_definitions_rx) {
                    let filters = filters
                        .iter()
                        .filter_map(|f| match f {
                            HttpFilterDefinition::StaticResponse(filter) => {
                                Some((filter.key().clone(), filter.clone()))
                            }
                            _ => None,
                        })
                        .collect();

                    tx.set(filters).await;
                }
                continue_on!(http_filter_definitions_rx.changed());
            }
        });

    rx
}

fn http_static_response_filter_body_cache(
    task_builder: &TaskBuilder,
    filters_rx: &Receiver<HashMap<HttpStaticResponseFilterKey, HttpStaticResponseFilter>>,
) -> HttpStaticResponseFilterBodyCache {
    let cache = HttpStaticResponseFilterBodyCache::new();

    task_builder
        .new_task(stringify!(http_static_response_filter_body_cache))
        .spawn({
            let cache = cache.clone();
            let filters_rx = filters_rx.clone();
            async move {
                loop {
                    if let ReadyState::Ready(_) = await_ready!(filters_rx) {
                        cache.clear();
                    }
                    continue_on!(filters_rx.changed());
                }
            }
        });

    cache
}

pub fn http_static_response_filter_handlers(
    task_builder: &TaskBuilder,
    http_filter_definitions_rx: &Receiver<Vec<HttpFilterDefinition>>,
    ipc_endpoint_rx: &Receiver<SocketAddr>,
    client: Arc<ClientWithMiddleware>,
    instance_context: Arc<InstanceContext>,
) -> Receiver<HashMap<HttpStaticResponseFilterKey, Arc<HttpStaticResponseFilterHandler>>> {
    let (tx, rx) = signal(stringify!(http_static_response_filter_handlers));
    let filters_rx = http_static_response_filters(task_builder, http_filter_definitions_rx);
    let cache = http_static_response_filter_body_cache(task_builder, &filters_rx);
    let ipc_endpoint_rx = ipc_endpoint_rx.clone();

    task_builder
        .new_task(stringify!(http_static_response_filter_handlers))
        .spawn(async move {
            loop {
                if let ReadyState::Ready((filters, ipc_endpoint)) =
                    await_ready!(filters_rx, ipc_endpoint_rx)
                {
                    let handlers = filters
                        .iter()
                        .map(|(key, filter)| {
                            let body = filter.body().as_ref().map(|body| {
                                let client = HttpStaticResponseFilterBodyCacheClient::builder()
                                    .filter_key(key.clone())
                                    .cache(cache.clone())
                                    .client(client.clone())
                                    .ipc_endpoint(*ipc_endpoint)
                                    .pod_name(instance_context.pod_name())
                                    .gateway_namespace(instance_context.gateway_namespace())
                                    .gateway_name(instance_context.gateway_name())
                                    .build();

                                HttpStaticResponseFilterHandlerBody::from(body, client)
                            });

                            let handler = HttpStaticResponseFilterHandler::from(filter, body);

                            (key.clone(), Arc::new(handler))
                        })
                        .collect();
                    tx.set(handlers).await;
                }
                continue_on!(filters_rx.changed(), ipc_endpoint_rx.changed());
            }
        });
    rx
}
