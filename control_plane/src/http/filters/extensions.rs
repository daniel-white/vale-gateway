
use std::collections::HashMap;
use std::sync::Arc;
use tokio::select;
use vg_core::await_ready;
use vg_core::http::filters::access_control::{HttpAccessControlFilter, HttpAccessControlFilterKey};
use vg_core::http::filters::client_addr::{HttpClientAddrFilter, HttpClientAddrFilterKey};
use vg_core::http::filters::error_response::{HttpErrorResponseFilter, HttpErrorResponseFilterKey};
use vg_core::http::filters::static_response::{HttpStaticResponseFilter, HttpStaticResponseFilterKey};
use vg_core::sync::signal::{Receiver, RecvError};
use vg_core::task::Builder as TaskBuilder;
use vg_core::ReadyState;
use super::access_control::controllers::http_access_control_filters;
use super::client_addr::controllers::http_client_addr_filters;
use super::error_response::controllers::http_error_response_filters;
use super::static_response::controllers::http_static_response_filters;
use crate::kubernetes::KubeClientCell;
use crate::options::Options;

#[derive(Debug, Clone)]
pub struct HttpExtensionFilters {
    access_control_filters_rx:
        Receiver<HashMap<HttpAccessControlFilterKey, Arc<HttpAccessControlFilter>>>,
    client_addr_filters_rx:
        Receiver<HashMap<HttpClientAddrFilterKey, Arc<HttpClientAddrFilter>>>,
    error_response_filters_rx:
        Receiver<HashMap<HttpErrorResponseFilterKey, Arc<HttpErrorResponseFilter>>>,
    static_response_filters_rx:
        Receiver<HashMap<HttpStaticResponseFilterKey, Arc<HttpStaticResponseFilter>>>,
}


impl HttpExtensionFilters {
    pub fn new(task_builder: &TaskBuilder, options: Arc<Options>, client: &Receiver<KubeClientCell>) -> Self {
        Self {
            access_control_filters_rx: http_access_control_filters(task_builder, options.clone(), client),
            client_addr_filters_rx: http_client_addr_filters(task_builder, options.clone(), client),
            error_response_filters_rx: http_error_response_filters(
                task_builder,
                options.clone(),
                client,
            ),
            static_response_filters_rx: http_static_response_filters(
                task_builder,
                options.clone(),
                client
            ),
        }
    }

    pub async fn ready(&self) -> ReadyState<&Self> {
        let access_control_filters_rx = self.access_control_filters_rx.clone();
        let client_addr_filters_rx = self.client_addr_filters_rx.clone();
        let error_response_filters_rx = self.error_response_filters_rx.clone();
        let static_response_filters_rx = self.static_response_filters_rx.clone();
        await_ready!(
            access_control_filters_rx,
            client_addr_filters_rx,
            error_response_filters_rx,
            static_response_filters_rx
        )
        .map(|_| self)
    }

    pub async fn changed(&self) -> Result<(), RecvError> {
        select! {
            _ = self.access_control_filters_rx.changed() => {},
            _ = self.client_addr_filters_rx.changed() => {},
            _ = self.error_response_filters_rx.changed() => {},
            _ = self.static_response_filters_rx.changed() => {},
        }
        Ok(())
    }

    pub async fn get_access_control_filter(
        &self,
        key: &HttpAccessControlFilterKey,
    ) -> Option<Arc<HttpAccessControlFilter>> {
        self.access_control_filters_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }

    pub async fn get_client_addr_filter(
        &self,
        key: &HttpClientAddrFilterKey,
    ) -> Option<Arc<HttpClientAddrFilter>> {
        self.client_addr_filters_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }

    pub async fn get_error_response_filter(
        &self,
        key: &HttpErrorResponseFilterKey,
    ) -> Option<Arc<HttpErrorResponseFilter>> {
        self.error_response_filters_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }

    pub async fn get_static_response_filter(
        &self,
        key: &HttpStaticResponseFilterKey,
    ) -> Option<Arc<HttpStaticResponseFilter>> {
        self.static_response_filters_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }
}
