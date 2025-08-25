use crate::http::filters::access_control::HttpAccessControlFilterHandler;
use crate::http::filters::client_addr::HttpClientAddrFilterHandler;
use crate::http::filters::error_response::HttpErrorResponseFilterHandler;
use crate::http::filters::headers::handler::HttpHeaderModifierFilterHandler;
use crate::http::filters::redirect_response::HttpRedirectResponseFilterHandler;
use crate::http::filters::HttpFilterHandlers;
use futures::{stream, StreamExt};
use std::sync::Arc;
use vg_core::http::listeners::HttpListenerFilter;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HttpListenerFilterHandler {
    UpstreamRequestHeaderModifier(Arc<HttpHeaderModifierFilterHandler>),
    ResponseHeaderModifier(Arc<HttpHeaderModifierFilterHandler>),
    RedirectResponse(Arc<HttpRedirectResponseFilterHandler>),
    AccessControl(Arc<HttpAccessControlFilterHandler>),
    ClientAddr(Arc<HttpClientAddrFilterHandler>),
    ErrorResponse(Arc<HttpErrorResponseFilterHandler>),
}

pub async fn collect_http_listener_filter_handlers(
    http_filter_handlers: &HttpFilterHandlers,
    http_listener_filters: &Vec<HttpListenerFilter>,
) -> Vec<HttpListenerFilterHandler> {
    stream::iter(http_listener_filters)
        .filter_map(async |filter| match filter {
            HttpListenerFilter::UpstreamRequestHeaderModifier(filter) => {
                let handler = HttpHeaderModifierFilterHandler::from(filter);
                Some(HttpListenerFilterHandler::UpstreamRequestHeaderModifier(
                    Arc::new(handler),
                ))
            }
            HttpListenerFilter::ResponseHeaderModifier(filter) => {
                let handler = HttpHeaderModifierFilterHandler::from(filter);
                Some(HttpListenerFilterHandler::ResponseHeaderModifier(Arc::new(
                    handler,
                )))
            }
            HttpListenerFilter::RedirectResponse(filter) => {
                let handler = HttpRedirectResponseFilterHandler::from(filter);
                Some(HttpListenerFilterHandler::RedirectResponse(Arc::new(
                    handler,
                )))
            }
            HttpListenerFilter::AccessControl(ref_) => {
                let handler = http_filter_handlers
                    .get_access_control_handler(ref_.key())
                    .await;

                handler.map(HttpListenerFilterHandler::AccessControl)
            }
            HttpListenerFilter::ClientAddr(ref_) => {
                let handler = http_filter_handlers
                    .get_client_addr_handler(ref_.key())
                    .await;

                handler.map(HttpListenerFilterHandler::ClientAddr)
            }
            HttpListenerFilter::ErrorResponse(ref_) => {
                let handler = http_filter_handlers
                    .get_error_response_handler(ref_.key())
                    .await;

                handler.map(HttpListenerFilterHandler::ErrorResponse)
            }
        })
        .collect()
        .await
}
