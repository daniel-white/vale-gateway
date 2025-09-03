use std::collections::HashMap;
use std::sync::Arc;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use tracing::debug;
use vg_core::{await_ready, continue_on, ReadyState};
use vg_core::sync::signal::{signal, Receiver};
use crate::options::Options;
use vg_core::task::Builder as TaskBuilder;
use crate::kubernetes::KubeClientCell;
use crate::kubernetes::objects::ObjectRef;
use crate::watch_objects;

pub fn endpoint_slices_by_service(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client: &Receiver<KubeClientCell>,
) -> Receiver<HashMap<ObjectRef, Vec<Arc<EndpointSlice>>>> {
    let (tx, rx) = signal(stringify!(endpoint_slices_by_service));
    let endpoint_slices_rx = watch_objects!(options, task_builder, EndpointSlice, client);

    task_builder
        .new_task(stringify!(endpoint_slices_by_service))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(endpoint_slices) = await_ready!(endpoint_slices_rx) {
                    let endpoint_slices_by_service = endpoint_slices
                        .iter()
                        .filter_map(|(_, _, endpoint_slice)| {
                            let metadata = &endpoint_slice.metadata;
                            let labels = metadata.labels.as_ref()?;
                            labels
                                .get("kubernetes.io/service-name")
                                .map(|service_name| {
                                    ObjectRef::of_kind::<Service>()
                                        .namespace(endpoint_slice.metadata.namespace.clone())
                                        .name(service_name)
                                        .build()
                                })
                                .map(|service_ref| (service_ref, endpoint_slice.clone()))
                        })
                        .fold(HashMap::new(), |mut acc, (service_ref, endpoint_slice)| {
                            acc.entry(service_ref)
                                .or_insert_with(Vec::new)
                                .push(endpoint_slice);
                            acc
                        });
                    tx.set(endpoint_slices_by_service).await;
                }

                continue_on!(endpoint_slices_rx.changed());
            }
        });

    rx
}