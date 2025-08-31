use super::HttpAccessControlFilterHandler;
use std::collections::HashMap;
use std::sync::Arc;
use vg_core::http::filters::access_control::{HttpAccessControlFilter, HttpAccessControlFilterKey};
use vg_core::http::listeners::HttpFilterDefinition;
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub type AccessControlFilterHandlers =
    HashMap<HttpAccessControlFilterKey, Arc<HttpAccessControlFilterHandler>>;

fn http_access_control_filters(
    task_builder: &TaskBuilder,
    http_filter_definitions_rx: &Receiver<Vec<HttpFilterDefinition>>,
) -> Receiver<HashMap<HttpAccessControlFilterKey, Arc<HttpAccessControlFilter>>> {
    let (tx, rx) = signal(stringify!(http_access_control_filters));
    let http_filter_definitions_rx = http_filter_definitions_rx.clone();

    task_builder
        .new_task(stringify!(http_access_control_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(http_filter_definitions_rx) {
                    let filters = filters
                        .iter()
                        .filter_map(|f| match f {
                            HttpFilterDefinition::AccessControl(filter) => {
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

pub fn http_access_control_filter_handlers(
    task_builder: &TaskBuilder,
    http_filter_definitions_rx: &Receiver<Vec<HttpFilterDefinition>>,
) -> Receiver<AccessControlFilterHandlers> {
    let (tx, rx) = signal(stringify!(http_access_control_filter_handlers));
    let filters_rx = http_access_control_filters(task_builder, http_filter_definitions_rx);

    task_builder
        .new_task(stringify!(http_access_control_filter_handlers))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(filters_rx) {
                    let handlers = filters
                        .iter()
                        .map(|(key, filter)| {
                            let handler = HttpAccessControlFilterHandler::from(filter);
                            (key.clone(), Arc::new(handler))
                        })
                        .collect();
                    tx.set(handlers).await;
                }
                continue_on!(filters_rx.changed());
            }
        });
    rx
}
