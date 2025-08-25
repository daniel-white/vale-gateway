use super::routes::{HttpRoute, HttpRouteMatch};
use crate::http::filters::access_control::HttpAccessControlFilterHandler;
use crate::http::filters::client_addr::HttpClientAddrFilterHandler;
use crate::http::filters::error_response::error_codes::ErrorResponseCode;
use crate::http::listener::filters::HttpListenerFilterHandler;
use crate::http::listener::filters::HttpListenerFilterHandler::*;
use crate::http::router::rules::HttpRouteRule;
use bytes::Bytes;
use http::{request, Response};
use std::collections::HashMap;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_core::http::routes::HttpRouteKey;
use crate::http::filters::headers::handler::HttpHeaderModifierFilterHandler;
use crate::http::router::rules::filters::HttpRouteRuleFilterHandler;

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpRouter {
    filters: Vec<HttpListenerFilterHandler>,
    routes: HashMap<HttpRouteKey, Arc<HttpRoute>>,
}

impl HttpRouter {
    pub fn client_addr_filter(&self) -> Option<Arc<HttpClientAddrFilterHandler>> {
        self.filters.iter().find_map(|f| {
            if let ClientAddr(handler) = f {
                Some(handler.clone())
            } else {
                None
            }
        })
    }

    pub fn access_control_filters(
        &self,
    ) -> impl Iterator<Item = Arc<HttpAccessControlFilterHandler>> {
        self.filters.iter().filter_map(|f| {
            if let AccessControl(handler) = f {
                Some(handler.clone())
            } else {
                None
            }
        })
    }

    pub fn match_route(&self, req: &request::Parts) -> Option<HttpRouteMatch> {
        self.routes.values().filter_map(|r| r.matches(req)).max()
    }

    pub fn generate_error_response(&self, code: ErrorResponseCode) -> Response<Option<Bytes>> {
        let generator = self
            .filters
            .iter()
            .filter_map(|f| {
                if let ErrorResponse(handler) = f {
                    Some(handler.clone())
                } else {
                    None
                }
            })
            .next()
            .unwrap_or_default();

        let response = generator.generate_response(code);

        self.response_header_modifiers(None);

        response
    }

    pub fn request_header_modifiers(
        &self,
        rule: Option<&HttpRouteRule>,
    ) -> Vec<Arc<HttpHeaderModifierFilterHandler>> {
        let mut header_modifiers = self.filters.iter().filter_map(|f| match f {
            UpstreamRequestHeaderModifier(handler) => Some(handler.clone()),
            _ => None,
        }).collect::<Vec<_>>();

        let mut rule_header_modifiers = rule
            .map_or(Vec::new(), |r| {
                r.filters().iter().filter_map(|f| match f {
                    HttpRouteRuleFilterHandler::UpstreamRequestHeaderModifier(handler) => Some(handler.clone()),
                    _ => None,
                }).collect()
            });

        header_modifiers.append(&mut rule_header_modifiers);

        header_modifiers
    }

    pub fn response_header_modifiers(
        &self,
        rule: Option<&HttpRouteRule>,
    ) -> Vec<Arc<HttpHeaderModifierFilterHandler>> {
        let mut router_header_modifiers: Vec<_> = self.filters.iter().filter_map(|f| match f {
            ResponseHeaderModifier(handler) => Some(handler.clone()),
            _ => None,
        }).collect();
        
        let mut header_modifiers = rule
            .map_or(Vec::new(), |r| {
                r.filters().iter().filter_map(|f| match f {
                    HttpRouteRuleFilterHandler::ResponseHeaderModifier(handler) => Some(handler.clone()),
                    _ => None,
                }).collect()
            });
        
        header_modifiers.append(&mut router_header_modifiers);
        
        header_modifiers
    }
}
