use crate::http::filters::access_control::{
    HttpAccessControlFilter, HttpAccessControlFilterKey, HttpAccessControlFilterRef,
};
use crate::http::filters::client_addr::{
    HttpClientAddrFilter, HttpClientAddrFilterKey, HttpClientAddrFilterRef,
};
use crate::http::filters::error_response::{
    HttpErrorResponseFilter, HttpErrorResponseFilterKey, HttpErrorResponseFilterRef,
};
use crate::http::filters::header_modifier::HttpHeaderModifierFilter;
use crate::http::filters::redirect_response::HttpRedirectResponseFilter;
use crate::http::filters::static_response::{
    HttpStaticResponseFilter, HttpStaticResponseFilterKey,
};
use crate::http::routes::{HttpRoute, HttpRouteBuilder, HttpRouteKey};
use crate::net::Port;
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::net::IpAddr;
use std::ops::Deref;
use std::sync::Arc;
use typed_builder::TypedBuilder;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HttpListenerProtocol {
    Http,
}

#[derive(Validate, Getters, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpListener {
    #[getset(get = "pub")]
    port: Port,

    #[getset(get = "pub")]
    filters: Vec<HttpListenerFilter>,

    #[getset(get = "pub")]
    filter_definitions: Vec<HttpFilterDefinition>,

    #[getset(get = "pub")]
    routes: Vec<HttpRoute>,

    #[getset(get = "pub")]
    backends: Vec<HttpBackend>,
}

impl HttpListener {
    pub fn builder() -> HttpListenerBuilder {
        HttpListenerBuilder {
            port: None,
            filters: Vec::new(),
            filter_definitions: Vec::new(),
            route_builders: Vec::new(),
            backends: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpListenerBuilder {
    port: Option<Port>,
    filters: Vec<HttpListenerFilter>,
    filter_definitions: Vec<HttpFilterDefinition>,
    route_builders: Vec<HttpRouteBuilder>,
    backends: Vec<HttpBackend>,
}

impl HttpListenerBuilder {
    pub fn build(self) -> HttpListener {
        HttpListener {
            port: self.port.expect("Listener port is not set"),
            filters: self.filters,
            filter_definitions: self.filter_definitions,
            routes: self
                .route_builders
                .into_iter()
                .map(HttpRouteBuilder::build)
                .collect(),
            backends: self.backends,
        }
    }

    pub fn port<P: Into<Port>>(&mut self, port: P) -> &mut Self {
        let port = port.into();
        self.port = Some(port);
        self
    }

    pub fn add_filter(&mut self, filter: HttpListenerFilter) -> &mut Self {
        self.filters.push(filter);
        self
    }

    pub fn add_filter_definition(&mut self, filter_definition: HttpFilterDefinition) -> &mut Self {
        self.filter_definitions.push(filter_definition);
        self
    }

    pub fn add_route<K, F>(&mut self, key: K, factory: F) -> &mut Self
    where
        K: Into<HttpRouteKey>,
        F: FnOnce(&mut HttpRouteBuilder),
    {
        let mut route_builder = HttpRoute::builder(key);
        factory(&mut route_builder);
        self.route_builders.push(route_builder);
        self
    }

    pub fn add_backend(&mut self, backend: HttpBackend) -> &mut Self {
        self.backends.push(backend);
        self
    }
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HttpListenerFilter {
    UpstreamRequestHeaderModifier(Arc<HttpHeaderModifierFilter>),
    ResponseHeaderModifier(Arc<HttpHeaderModifierFilter>),
    RedirectResponse(Arc<HttpRedirectResponseFilter>),
    AccessControl(HttpAccessControlFilterRef),
    ClientAddr(HttpClientAddrFilterRef),
    ErrorResponse(HttpErrorResponseFilterRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HttpFilterDefinitionKey {
    StaticResponse(HttpStaticResponseFilterKey),
    AccessControl(HttpAccessControlFilterKey),
    ClientAddr(HttpClientAddrFilterKey),
    ErrorResponse(HttpErrorResponseFilterKey),
}

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HttpFilterDefinition {
    StaticResponse(Arc<HttpStaticResponseFilter>),
    AccessControl(Arc<HttpAccessControlFilter>),
    ClientAddr(Arc<HttpClientAddrFilter>),
    ErrorResponse(Arc<HttpErrorResponseFilter>),
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpBackend {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    kind: String,
    #[getset(get = "pub")]
    #[builder(setter(into))]
    name: String,
    #[getset(get = "pub")]
    #[builder(setter(into))]
    namespace: String,
    #[getset(get = "pub")]
    endpoints: Vec<HttpBackendEndpoint>,
}

#[derive(
    Validate, Getters, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct HttpBackendEndpoint {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(setter(into))]
    node: Option<String>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(setter(into))]
    zone: Option<String>,

    #[getset(get = "pub")]
    addrs: Vec<IpAddr>,
}
