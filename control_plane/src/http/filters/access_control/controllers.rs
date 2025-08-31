use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use std::collections::HashMap;
use std::sync::Arc;
use vg_api::v1alpha1::{AccessControlFilter, AccessControlFilterEffect};
use vg_core::http::filters::access_control::{
    HttpAccessControlClients, HttpAccessControlEffect, HttpAccessControlFilter,
    HttpAccessControlFilterKey,
};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_access_control_filters(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client_rx: &Receiver<KubeClientCell>,
) -> Receiver<HashMap<HttpAccessControlFilterKey, Arc<HttpAccessControlFilter>>> {
    let (tx, rx) = signal(stringify!(http_access_control_filters));
    let access_control_filters_rx =
        watch_objects!(options, task_builder, AccessControlFilter, client_rx);

    task_builder
        .new_task(stringify!(http_access_control_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(access_control_filters_rx) {
                    let filters = filters
                        .iter()
                        .map(|(_, _, f)| convert(f.as_ref()))
                        .collect();

                    tx.set(filters).await;
                }
                continue_on!(access_control_filters_rx.changed());
            }
        });

    rx
}

fn convert(
    filter: &AccessControlFilter,
) -> (HttpAccessControlFilterKey, Arc<HttpAccessControlFilter>) {
    let key = format!(
        "{}-{}",
        filter.metadata.name.as_deref().unwrap(),
        filter.metadata.namespace.as_deref().unwrap()
    );
    let key: HttpAccessControlFilterKey = key.into();
    let filter = &filter.spec;

    let effect = match filter.effect {
        AccessControlFilterEffect::Allow => HttpAccessControlEffect::Allow,
        AccessControlFilterEffect::Deny => HttpAccessControlEffect::Deny,
    };

    let clients = HttpAccessControlClients::builder()
        .ip_ranges(filter.clients.ip_ranges.clone())
        .ips(filter.clients.ips.clone())
        .build();

    (
        key.clone(),
        Arc::new(
            HttpAccessControlFilter::builder()
                .key(key)
                .effect(effect)
                .clients(clients)
                .build(),
        ),
    )
}
