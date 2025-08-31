use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use http::{HeaderValue, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use vg_api::v1alpha1::StaticResponseFilter;
use vg_core::http::filters::static_response::{
    HttpStaticResponseBody, HttpStaticResponseFilter, HttpStaticResponseFilterKey,
};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_static_response_filters(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client_rx: &Receiver<KubeClientCell>,
) -> Receiver<HashMap<HttpStaticResponseFilterKey, Arc<HttpStaticResponseFilter>>> {
    let (tx, rx) = signal(stringify!(http_static_response_filters));
    let static_response_filters_rx =
        watch_objects!(options, task_builder, StaticResponseFilter, client_rx);

    task_builder
        .new_task(stringify!(http_error_response_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(static_response_filters_rx) {
                    let filters = filters
                        .iter()
                        .map(|(_, _, f)| convert(f.as_ref()))
                        .collect();

                    tx.set(filters).await;
                }
                continue_on!(static_response_filters_rx.changed());
            }
        });

    rx
}

fn convert(
    filter: &StaticResponseFilter,
) -> (HttpStaticResponseFilterKey, Arc<HttpStaticResponseFilter>) {
    let key = format!(
        "{}-{}",
        filter.metadata.name.as_deref().unwrap(),
        filter.metadata.namespace.as_deref().unwrap()
    );
    let key: HttpStaticResponseFilterKey = key.into();
    let metadata = &filter.metadata;
    let name: &str = metadata.name.as_deref().unwrap();
    let resource_version = metadata.resource_version.as_deref().unwrap();
    let filter = &filter.spec;

    let status_code = StatusCode::from_u16(filter.status_code).unwrap();

    let body = filter.body.as_ref().map(|body| {
        let key = format!("{}-{}", key, resource_version);
        let content_type = HeaderValue::from_str(body.content_type.as_str()).unwrap();

        HttpStaticResponseBody::builder()
            .key(key)
            .content_type(content_type)
            .build()
    });

    let filter = HttpStaticResponseFilter::builder()
        .key(name)
        .status_code(status_code)
        .body(body)
        .build();
    
    (key, Arc::new(filter))
}
