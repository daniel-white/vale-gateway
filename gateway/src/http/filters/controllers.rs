use vg_core::http::listeners::{HttpFilterDefinition, HttpListener};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_filter_definitions(
    task_builder: &TaskBuilder,
    http_listener_rx: &Receiver<Option<HttpListener>>,
) -> Receiver<Vec<HttpFilterDefinition>> {
    let (tx, rx) = signal(stringify!(http_filter_definitions));
    let http_listener_rx = http_listener_rx.clone();

    task_builder
        .new_task(stringify!(http_filter_definitions))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(http_listener) = await_ready!(http_listener_rx) {
                    let filter_definitions = http_listener
                        .as_ref()
                        .map(|listener| listener.filter_definitions().clone())
                        .unwrap_or_default();
                    tx.set(filter_definitions).await;
                }
                continue_on!(http_listener_rx.changed());
            }
        });

    rx
}
