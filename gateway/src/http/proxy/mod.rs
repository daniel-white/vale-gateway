mod context;

use crate::http::filters::error_response::error_codes::ErrorResponseCode;
use crate::http::instrumentation::HttpRequestInstrumentation;
use crate::http::proxy::context::{AccessControlEnforcementResult, HttpRouteMatchResult};
use crate::http::router::router::HttpRouter;
use async_trait::async_trait;
use context::HttpRequestContext;
use http::header::SERVER;
use http::{request, HeaderMap, HeaderName, HeaderValue, StatusCode};
use pingora::http::ResponseHeader;
use pingora::prelude::*;
use pingora::protocols::http::error_resp::gen_error_response;
use std::sync::Arc;
use tracing::{debug, instrument, warn};
use typed_builder::TypedBuilder;
use vg_core::sync::signal::Receiver;
use crate::http::filters::HttpHeaders;

#[derive(TypedBuilder)]
pub struct HttpProxy {
    http_router_rx: Receiver<Arc<HttpRouter>>,
}

#[async_trait]
impl ProxyHttp for HttpProxy {
    type CTX = HttpRequestContext;

    fn new_ctx(&self) -> Self::CTX {
        let instrumentation = HttpRequestInstrumentation::new();

        HttpRequestContext::builder()
            .instrumentation(instrumentation)
            .http_router_rx(self.http_router_rx.clone())
            .build()
    }

    #[instrument(name = "upstream_peer", parent = ctx.instrumentation().request_span(), skip(self, _session, ctx))]
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        Err(Error::explain(
                    HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
                    "Missing configuration",
                ))
        
        // match ctx.next_upstream_peer() {
        //     UpstreamPeerResult::Addr(addr) => {
        //         ctx.instrumentation().record_upstream_peer(addr);
        //         Ok(Box::new(HttpPeer::new(addr, false, "".to_string())))
        //     }
        //     UpstreamPeerResult::NotFound => {
        //         ctx.instrumentation().record_status(StatusCode::NOT_FOUND);
        //         Err(Error::explain(
        //             HTTPStatus(StatusCode::NOT_FOUND.into()),
        //             "No matching route found",
        //         ))
        //     }
        //     UpstreamPeerResult::ServiceUnavailable => {
        //         ctx.instrumentation()
        //             .record_status(StatusCode::SERVICE_UNAVAILABLE);
        //         Err(Error::explain(
        //             HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
        //             "Service unavailable",
        //         ))
        //     }
        //     UpstreamPeerResult::MissingConfiguration => {
        //         ctx.instrumentation()
        //             .record_status(StatusCode::SERVICE_UNAVAILABLE);
        //         Err(Error::explain(
        //             HTTPStatus(StatusCode::SERVICE_UNAVAILABLE.into()),
        //             "Missing configuration",
        //         ))
        //     }
        // }
    }

    #[instrument(name = "request_filter", parent = ctx.instrumentation().request_span(), skip(self, session, ctx))]
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let route = ctx.match_route(session.req_header()).await;

        let match_ = match route {
            HttpRouteMatchResult::Found(match_) => match_,
            HttpRouteMatchResult::NotFound => {
                self.send_error_response(session, ctx, ErrorResponseCode::NoRoute)
                    .await?;
                return Ok(true);
            }
            HttpRouteMatchResult::NotReady => {
                self.send_error_response(session, ctx, ErrorResponseCode::MissingConfiguration)
                    .await?;
                return Ok(true);
            }
        };
        
        ctx.apply_headers_to_request(session).await;

         Ok(false)

        // let error_code = match route {
        //     MatchRouteResult::Found(route, rule, matched_prefix) => {
        //         // if let ReadyState::Ready(handlers) =
        //         //     await_ready!(access_control_filters_handlers_rx)
        //         // {
        //         //
        //
        //         let static_responses_rx = self.static_responses_rx.clone();
        //         for filter in rule.filters() {
        //             if let Some(ext_static_response) = &filter.ext_static_response
        //                 && let ReadyState::Ready(static_responses) =
        //                     await_ready!(static_responses_rx)
        //             {
        //                 let static_filter = StaticResponseFilter::builder()
        //                     .responses(static_responses.clone())
        //                     .static_response_bodies(self.static_response_bodies_cache.clone())
        //                     .build();
        //
        //                 match static_filter
        //                     .apply_to_session(session, ext_static_response.key())
        //                     .await
        //                 {
        //                     Ok(Some(status_code)) => {
        //                         debug!(
        //                             "Applied static response filter for route: {:?} with key: {}",
        //                             route,
        //                             ext_static_response.key()
        //                         );
        //                         ctx.instrumentation().record_status(status_code);
        //                         return Ok(true);
        //                     }
        //                     Ok(None) => {
        //                         debug!(
        //                             "Static response key '{}' not found in configuration",
        //                             ext_static_response.key()
        //                         );
        //                     }
        //                     Err(e) => {
        //                         warn!(
        //                             "Failed to apply static response filter for key '{}': {}",
        //                             ext_static_response.key(),
        //                             e
        //                         );
        //                     }
        //                 }
        //             }
        //         }
        //
        //         // Check for redirect filters before proceeding to upstream
        //         for filter in rule.filters() {
        //             if let Some(request_redirect) = &filter.request_redirect {
        //                 let redirect_filter = RequestRedirectFilter::new(request_redirect.clone());
        //
        //                 // Create route match context with the matched prefix
        //                 let route_context =
        //                     crate::proxy::filters::request_redirect::RouteMatchContext {
        //                         matched_prefix: matched_prefix.clone(),
        //                     };
        //
        //                 if let Ok(Some(redirect_response)) = redirect_filter
        //                     .apply_to_pingora_request_with_context(
        //                         session.req_header(),
        //                         &route_context,
        //                     )
        //                 {
        //                     debug!(
        //                         "Applying redirect filter for route: {:?} with prefix: {:?}",
        //                         route, matched_prefix
        //                     );
        //                     ctx.instrumentation()
        //                         .record_status(redirect_response.status_code);
        //
        //                     // Generate redirect response
        //                     let mut redirect_resp =
        //                         gen_error_response(redirect_response.status_code.as_u16());
        //                     self.set_response_server_header(&mut redirect_resp)?;
        //                     redirect_resp.insert_header("Location", &redirect_response.location)?;
        //
        //                     session.write_response_header_ref(&redirect_resp).await?;
        //                     session
        //                         .write_response_body(Some(bytes::Bytes::new()), true)
        //                         .await?;
        //
        //                     return Ok(true); // Request handled, don't proceed to upstream
        //                 }
        //             }
        //         }
        //
        //         // Apply URL upstream_uri_rewrite filters after redirect checks
        //         for filter in rule.filters() {
        //             if let Some(url_rewrite) = &filter.url_rewrite {
        //                 let rewrite_filter = URLRewriteFilter::new(url_rewrite.clone());
        //
        //                 // Create route match context with the matched prefix (reuse from redirect)
        //                 let route_context =
        //                     crate::proxy::filters::request_redirect::RouteMatchContext {
        //                         matched_prefix: matched_prefix.clone(),
        //                     };
        //
        //                 // Apply URL upstream_uri_rewrite to the request headers
        //                 if let Ok(was_rewritten) = rewrite_filter
        //                     .apply_to_pingora_request_with_context(
        //                         session.req_header_mut(),
        //                         &route_context,
        //                     )
        //                 {
        //                     if was_rewritten {
        //                         debug!(
        //                             "Applied URL upstream_uri_rewrite filter for route: {:?} with prefix: {:?}",
        //                             route, matched_prefix
        //                         );
        //                     }
        //                 } else {
        //                     warn!(
        //                         "Failed to apply URL upstream_uri_rewrite filter for route: {:?}",
        //                         route
        //                     );
        //                 }
        //             }
        //         }
        //
        //         ctx.set(
        //             MatchRouteResult::Found(route, rule, matched_prefix),
        //             client_addr,
        //         );
        //         return Ok(false);
        //     }
        //     MatchRouteResult::NotFound => ErrorResponseCode::NoRoute,
        //     MatchRouteResult::MissingConfiguration => ErrorResponseCode::MissingConfiguration,
        // };
        //
        // let response = ctx.generate_error_response(error_code).await;
        //
        // ctx.instrumentation().record_status(response.status());
        // let mut error_response = gen_error_response(response.status().into());
        // self.set_response_server_header(&mut error_response)?;
        // for (name, value) in response.headers() {
        //     error_response.insert_header(name, value)?;
        // }
        //
        // session.write_response_header_ref(&error_response).await?;
        // session
        //     .write_response_body(response.body().clone(), true)
        //     .await?;
    }

    #[instrument(name = "early_request_filter", parent = ctx.instrumentation().request_span(), skip(self, session, ctx))]
    async fn early_request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        ctx.instrumentation().record_request(session.req_header());

        let addr = session.client_addr().and_then(|a| a.as_inet()).cloned();
        if let Some(addr) = addr {
            // HACK: Temporarily cast away the immutability of the request parts to allow modifying headers
            let req: &request::Parts = session.req_header();
            #[allow(invalid_reference_casting)]
            unsafe {
                let req: *const request::Parts = req;
                let req = req as *mut request::Parts;
                ctx.identify_client_addr(addr, &mut *req).await;
            };
        }

        match ctx.enforce_access_control(session.req_header()).await {
            AccessControlEnforcementResult::NotReady => {
                self.send_error_response(session, ctx, ErrorResponseCode::MissingConfiguration)
                    .await
            }
            AccessControlEnforcementResult::Denied => {
                self.send_error_response(session, ctx, ErrorResponseCode::MissingConfiguration)
                    .await
            }
            AccessControlEnforcementResult::Allowed => {
                debug!(
                    "Access control filters allowed request for client: {:?}",
                    addr
                );

                Ok(())
            }
        }
    }

    #[instrument(name = "upstream_request_filter", parent = ctx.instrumentation().request_span(), skip(self, _session, upstream_request, ctx))]
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let mut upstream_req_headers = HeaderMap::new();
        ctx.instrumentation()
            .begin_upstream_call(&mut upstream_req_headers);
        for (header_name, header_value) in upstream_req_headers.iter() {
            let _ = upstream_request.insert_header(
                header_name.as_str().to_string(),
                header_value.to_str().unwrap_or("").to_string(),
            );
        }

        Ok(())
    }

    #[instrument(name = "upstream_response_filter", parent = ctx.instrumentation().request_span(), skip(self, _session, upstream_response, ctx))]
    fn upstream_response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        ctx.instrumentation().end_upstream_call(upstream_response);

        Ok(())
    }

    #[instrument(name = "response_filter", parent = ctx.instrumentation().request_span(), skip(self, _session, upstream_response, ctx))]
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        ctx.instrumentation()
            .record_status(upstream_response.status);

        self.set_response_server_header(upstream_response)?;

        //let response: &mut Response<_> = upstream_response.deref_mut().as();
        ctx.apply_headers_to_response(upstream_response).await;

        Ok(())
    }
}

impl HttpProxy {
    fn set_response_server_header(&self, response: &mut ResponseHeader) -> Result<(), BError> {
        response.insert_header(SERVER, "Vale Gateway")?;
        Ok(())
    }

    async fn send_error_response(
        &self,
        session: &mut Session,
        ctx: &HttpRequestContext,
        code: ErrorResponseCode,
    ) -> Result<()> {
        let response = ctx.generate_error_response(code).await;

        ctx.instrumentation().record_status(response.status());
        let mut error_response = gen_error_response(response.status().into());
        self.set_response_server_header(&mut error_response)?;
        for (name, value) in response.headers() {
            error_response.insert_header(name, value)?;
        }

        session.write_response_header_ref(&error_response).await?;
        session
            .write_response_body(response.body().clone(), true)
            .await?;

        Ok(())
    }
}

impl HttpHeaders for ResponseHeader {
    fn remove(&mut self, header: &HeaderName) {
        self.remove_header(header);
    }

    fn insert(&mut self, header: HeaderName, value: HeaderValue) {
        let _ = self.insert_header(header, value);
    }

    fn append(&mut self, header: HeaderName, value: HeaderValue) {
        let _ = self.append_header(header, value);
    }
}

impl HttpHeaders for Session {
    fn remove(&mut self, header: &HeaderName) {
        self.req_header_mut().remove_header(header);
    }

    fn insert(&mut self, header: HeaderName, value: HeaderValue) {
        let _ = self.req_header_mut().insert_header(header, value);
    }

    fn append(&mut self, header: HeaderName, value: HeaderValue) {
        let _ = self.req_header_mut().append_header(header, value);
    }
}
