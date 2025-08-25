use crate::http::filters::error_response::error_codes::ErrorResponseCode;
use crate::http::filters::HttpHeaders;
use crate::http::instrumentation::HttpRequestInstrumentation;
use crate::http::router::router::HttpRouter;
use crate::http::router::routes::HttpRouteMatch;
use bytes::Bytes;
use getset::Getters;
use http::{request, Response, StatusCode};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::OnceCell;
use typed_builder::TypedBuilder;
use vg_core::sync::signal::Receiver;

#[derive(Debug)]
pub enum HttpUpstreamPeerResult {
    Addr(SocketAddr),
    NotFound,
    ServiceUnavailable,
    MissingConfiguration,
}

#[derive(TypedBuilder, Getters, Debug)]
pub struct HttpRequestContext {
    #[getset(get = "pub")]
    instrumentation: HttpRequestInstrumentation,

    http_router_rx: Receiver<Arc<HttpRouter>>,

    #[builder(default)]
    router: OnceCell<Option<Arc<HttpRouter>>>,

    #[builder(default)]
    matched_route: OnceCell<HttpRouteMatchResult>,

    #[builder(default)]
    client_addr: OnceCell<Option<IpAddr>>,
}

unsafe impl Send for HttpRequestContext {}

unsafe impl Sync for HttpRequestContext {}

pub enum AccessControlEnforcementResult {
    NotReady,
    Allowed,
    Denied,
}

#[derive(Debug, PartialEq)]
pub enum HttpRouteMatchResult {
    Found(HttpRouteMatch),
    NotFound,
    NotReady,
}

impl HttpRequestContext {
    pub async fn router(&self) -> Option<Arc<HttpRouter>> {
        self.router
            .get_or_init(|| async { self.http_router_rx.get().await.as_ref().cloned() })
            .await
            .as_ref()
            .cloned()
    }

    pub async fn identify_client_addr(&self, addr: SocketAddr, req: &mut request::Parts) {
        self.client_addr
            .get_or_init(|| async {
                self.router()
                    .await
                    .and_then(|r| r.client_addr_filter())
                    .and_then(|f| f.filter(addr, req))
                    .inspect(|client_addr| self.instrumentation().record_client_addr(*client_addr))
            })
            .await;
    }

    pub fn client_addr(&self) -> Option<IpAddr> {
        self.client_addr.get().copied().flatten()
    }

    pub async fn enforce_access_control(
        &self,
        _req: &request::Parts,
    ) -> AccessControlEnforcementResult {
        let client_addr = self.client_addr();
        let router = self.router().await;

        match (client_addr, router.as_deref()) {
            (_, None) => AccessControlEnforcementResult::NotReady,
            (Some(client_addr), Some(router)) => {
                let is_allowed = router
                    .access_control_filters()
                    .all(|f| f.evaluate(client_addr).is_allowed());

                if is_allowed {
                    AccessControlEnforcementResult::Allowed
                } else {
                    AccessControlEnforcementResult::Denied
                }
            }
            (None, _) => AccessControlEnforcementResult::Denied, // No client IP, deny access
        }
    }

    pub async fn match_route(&self, req: &request::Parts) -> &HttpRouteMatchResult {
        self.matched_route
            .get_or_init(|| async {
                let router = self.router().await;
                match router.as_deref() {
                    Some(router) => match router.match_route(req) {
                        Some(matched) => HttpRouteMatchResult::Found(matched),
                        None => HttpRouteMatchResult::NotFound,
                    },
                    None => HttpRouteMatchResult::NotReady,
                }
            })
            .await
    }

    pub fn route(&self) -> &HttpRouteMatchResult {
        self.matched_route
            .get()
            .unwrap_or_else(|| &HttpRouteMatchResult::NotReady)
    }

    pub async fn generate_error_response(
        &self,
        code: ErrorResponseCode,
    ) -> Response<Option<Bytes>> {
        let router = self.router().await;
        if let Some(router) = router.as_deref() {
            return router.generate_error_response(code);
        }

        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(None)
            .unwrap()
    }

    pub async fn apply_headers_to_request(&self, headers: &mut impl HttpHeaders) {
        let rule = match self.route() {
            HttpRouteMatchResult::Found(m) => Some(m.rule().clone()),
            _ => None,
        };

        if let Some(router) = self.router().await.as_deref() {
            for modifier in router.request_header_modifiers(rule.as_deref()) {
                modifier.apply(headers);
            }
        }
    }

    pub async fn apply_headers_to_response(&self, headers: &mut impl HttpHeaders) {
        let rule = match self.route() {
            HttpRouteMatchResult::Found(m) => Some(m.rule().clone()),
            _ => None,
        };

        if let Some(router) = self.router().await.as_deref() {
            for modifier in router.response_header_modifiers(rule.as_deref()) {
                modifier.apply(headers);
            }
        }
    }
}
