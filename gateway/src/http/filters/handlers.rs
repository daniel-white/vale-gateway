use super::access_control::{
    http_access_control_filter_handlers, HttpAccessControlFilterHandler, HttpAccessControlFilterKey,
};
use super::client_addr::{
    http_client_addr_filter_handlers, HttpClientAddrFilterHandler, HttpClientAddrFilterKey,
};
use super::controllers::http_filter_definitions;
use super::error_response::{
    http_error_response_filter_handlers, HttpErrorResponseFilterHandler, HttpErrorResponseFilterKey,
};
use crate::http::filters::static_response::{
    http_static_response_filter_handlers, HttpStaticResponseFilterHandler,
    HttpStaticResponseFilterKey,
};
use crate::infra::InstanceContext;
use reqwest_middleware::ClientWithMiddleware;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::select;
use typed_builder::TypedBuilder;
use vg_core::await_ready;
use vg_core::http::listeners::HttpListener;
use vg_core::sync::signal::{Receiver, RecvError};
use vg_core::task::Builder as TaskBuilder;
use vg_core::ReadyState;

#[derive(Debug, Clone)]
pub struct HttpFilterHandlers {
    access_control_filter_handlers_rx:
        Receiver<HashMap<HttpAccessControlFilterKey, Arc<HttpAccessControlFilterHandler>>>,
    client_addr_filter_handler_rx:
        Receiver<HashMap<HttpClientAddrFilterKey, Arc<HttpClientAddrFilterHandler>>>,
    error_response_filter_handler_rx:
        Receiver<HashMap<HttpErrorResponseFilterKey, Arc<HttpErrorResponseFilterHandler>>>,
    static_response_filter_handler_rx:
        Receiver<HashMap<HttpStaticResponseFilterKey, Arc<HttpStaticResponseFilterHandler>>>,
}

#[derive(Debug, TypedBuilder)]
pub struct HttpFilterHandlersDependencies {
    http_listener_rx: Receiver<Option<HttpListener>>,
    ipc_endpoint: Receiver<SocketAddr>,
    client: Arc<ClientWithMiddleware>,
    instance_context: Arc<InstanceContext>,
}

impl HttpFilterHandlers {
    pub fn new(task_builder: &TaskBuilder, dependencies: HttpFilterHandlersDependencies) -> Self {
        let http_filter_definitions_rx =
            http_filter_definitions(task_builder, &dependencies.http_listener_rx);

        Self {
            access_control_filter_handlers_rx: http_access_control_filter_handlers(
                task_builder,
                &http_filter_definitions_rx,
            ),
            client_addr_filter_handler_rx: http_client_addr_filter_handlers(
                task_builder,
                &http_filter_definitions_rx,
            ),
            error_response_filter_handler_rx: http_error_response_filter_handlers(
                task_builder,
                &http_filter_definitions_rx,
            ),
            static_response_filter_handler_rx: http_static_response_filter_handlers(
                task_builder,
                &http_filter_definitions_rx,
                &dependencies.ipc_endpoint,
                dependencies.client,
                dependencies.instance_context,
            ),
        }
    }

    pub async fn ready(&self) -> ReadyState<&Self> {
        let access_control_filter_handlers_rx = self.access_control_filter_handlers_rx.clone();
        let client_addr_filter_handler_rx = self.client_addr_filter_handler_rx.clone();
        let error_response_filter_handler_rx = self.error_response_filter_handler_rx.clone();
        let static_response_filter_handler_rx = self.static_response_filter_handler_rx.clone();
        await_ready!(
            access_control_filter_handlers_rx,
            client_addr_filter_handler_rx,
            error_response_filter_handler_rx,
            static_response_filter_handler_rx
        )
        .map(|_| self)
    }

    pub async fn changed(&self) -> Result<(), RecvError> {
        select! {
            _ = self.access_control_filter_handlers_rx.changed() => {},
            _ = self.client_addr_filter_handler_rx.changed() => {},
            _ = self.error_response_filter_handler_rx.changed() => {},
            _ = self.static_response_filter_handler_rx.changed() => {},
        }
        Ok(())
    }

    pub async fn get_access_control_handler(
        &self,
        key: &HttpAccessControlFilterKey,
    ) -> Option<Arc<HttpAccessControlFilterHandler>> {
        self.access_control_filter_handlers_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }

    pub async fn get_client_addr_handler(
        &self,
        key: &HttpClientAddrFilterKey,
    ) -> Option<Arc<HttpClientAddrFilterHandler>> {
        self.client_addr_filter_handler_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }

    pub async fn get_error_response_handler(
        &self,
        key: &HttpErrorResponseFilterKey,
    ) -> Option<Arc<HttpErrorResponseFilterHandler>> {
        self.error_response_filter_handler_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }

    pub async fn get_static_response_handler(
        &self,
        key: &HttpStaticResponseFilterKey,
    ) -> Option<Arc<HttpStaticResponseFilterHandler>> {
        self.static_response_filter_handler_rx
            .get()
            .await
            .as_ref()
            .and_then(|m| m.get(key))
            .cloned()
    }
}
