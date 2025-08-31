use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;
use vg_api::v1alpha1::{ErrorResponseFilter, ErrorResponseFilterKind};
use vg_core::http::filters::error_response::{
    HttpErrorResponseFilter, HttpErrorResponseFilterKey, HttpErrorResponseKind,
    HttpProblemDetailErrorResponse,
};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_error_response_filters(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client_rx: &Receiver<KubeClientCell>,
) -> Receiver<HashMap<HttpErrorResponseFilterKey, Arc<HttpErrorResponseFilter>>> {
    let (tx, rx) = signal(stringify!(http_error_response_filters));
    let error_response_filters_rx =
        watch_objects!(options, task_builder, ErrorResponseFilter, client_rx);

    task_builder
        .new_task(stringify!(http_error_response_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(error_response_filters_rx) {
                    let filters = filters
                        .iter()
                        .map(|(_, _, f)| convert(f.as_ref()))
                        .collect();

                    tx.set(filters).await;
                }
                continue_on!(error_response_filters_rx.changed());
            }
        });

    rx
}

fn convert(
    filter: &ErrorResponseFilter,
) -> (HttpErrorResponseFilterKey, Arc<HttpErrorResponseFilter>) {
    let key = format!(
        "{}-{}",
        filter.metadata.name.as_deref().unwrap(),
        filter.metadata.namespace.as_deref().unwrap()
    );
    let filter = &filter.spec;
    let key: HttpErrorResponseFilterKey = key.into();

    let builder = HttpErrorResponseFilter::builder().key(key.clone());
    let filter = match (&filter.kind, &filter.problem_detail) {
        (ErrorResponseFilterKind::Empty, _) => builder
            .kind(HttpErrorResponseKind::Empty)
            .problem_detail(None)
            .build(),
        (ErrorResponseFilterKind::Html, _) => builder
            .kind(HttpErrorResponseKind::Html)
            .problem_detail(None)
            .build(),
        (ErrorResponseFilterKind::ProblemDetail, None) => builder
            .kind(HttpErrorResponseKind::ProblemDetail)
            .problem_detail(None)
            .build(),
        (ErrorResponseFilterKind::ProblemDetail, Some(problem_detail)) => {
            let problem_detail = HttpProblemDetailErrorResponse::builder()
                .authority(
                    problem_detail
                        .authority
                        .as_ref()
                        .and_then(|authority| Url::parse(authority).ok()),
                )
                .build();
            builder
                .kind(HttpErrorResponseKind::ProblemDetail)
                .problem_detail(Some(problem_detail))
                .build()
        }
    };

    (key, Arc::new(filter))
}
