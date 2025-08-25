use crate::http::filters::headers::handler::HttpHeaderModifierFilterHandler;
use crate::http::filters::redirect_response::HttpRedirectResponseFilterHandler;
use crate::http::filters::static_response::HttpStaticResponseFilterHandler;
use crate::http::filters::upstream_uri_rewrite::HttpUpstreamUriRewriteFilterHandler;
use crate::http::filters::HttpFilterHandlers;
use futures::{stream, StreamExt};
use std::sync::Arc;
use vg_core::http::routes::rules::HttpRouteRuleFilter;

#[derive(Debug, PartialEq, Eq)]
pub enum HttpRouteRuleFilterHandler {
    UpstreamUriRewrite(Arc<HttpUpstreamUriRewriteFilterHandler>),
    UpstreamRequestHeaderModifier(Arc<HttpHeaderModifierFilterHandler>),
    RedirectResponse(Arc<HttpRedirectResponseFilterHandler>),
    ResponseHeaderModifier(Arc<HttpHeaderModifierFilterHandler>),
    StaticResponse(Arc<HttpStaticResponseFilterHandler>),
}

pub async fn collect_http_route_rule_filter_handlers(
    filters: &[HttpRouteRuleFilter],
    http_filter_handlers: &HttpFilterHandlers,
) -> Vec<HttpRouteRuleFilterHandler> {
    stream::iter(filters)
        .filter_map(async |filter| {
            match filter {
                HttpRouteRuleFilter::UpstreamUriRewrite(filter) => {
                    let handler = HttpUpstreamUriRewriteFilterHandler::from(filter);
                    Some(HttpRouteRuleFilterHandler::UpstreamUriRewrite(Arc::new(
                        handler,
                    )))
                }
                HttpRouteRuleFilter::UpstreamRequestHeaderModifier(filter) => {
                    let handler = HttpHeaderModifierFilterHandler::from(filter);
                    Some(HttpRouteRuleFilterHandler::UpstreamRequestHeaderModifier(
                        Arc::new(handler),
                    ))
                }
                HttpRouteRuleFilter::RedirectResponse(filter) => {
                    let handler = HttpRedirectResponseFilterHandler::from(filter);
                    Some(HttpRouteRuleFilterHandler::RedirectResponse(Arc::new(
                        handler,
                    )))
                }
                HttpRouteRuleFilter::ResponseHeaderModifier(filter) => {
                    let handler = HttpHeaderModifierFilterHandler::from(filter);
                    Some(HttpRouteRuleFilterHandler::ResponseHeaderModifier(
                        Arc::new(handler),
                    ))
                }
                HttpRouteRuleFilter::StaticResponse(ref_) => {
                    let handler = http_filter_handlers
                        .get_static_response_handler(ref_.key())
                        .await;

                    handler.map(HttpRouteRuleFilterHandler::StaticResponse)
                }
            }
        })
        .collect()
        .await
}
