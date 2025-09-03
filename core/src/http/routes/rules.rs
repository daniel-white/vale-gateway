use super::backends::HttpRouteBackend;
use crate::http::filters::header_modifier::HttpHeaderModifierFilter;
use crate::http::filters::redirect_response::HttpRedirectResponseFilter;
use crate::http::filters::static_response::HttpStaticResponseFilterRef;
use crate::http::filters::upstream_uri_rewrite::HttpUpstreamUriRewriteFilter;
use crate::http::matches::{HttpRequestMatches, HttpRouteRuleMatchesBuilder};
use getset::Getters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_valid::Validate;
use std::sync::Arc;

#[derive(Validate, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum HttpRouteRuleFilter {
    UpstreamUriRewrite(Arc<HttpUpstreamUriRewriteFilter>),
    UpstreamRequestHeaderModifier(Arc<HttpHeaderModifierFilter>),
    RedirectResponse(Arc<HttpRedirectResponseFilter>),
    ResponseHeaderModifier(Arc<HttpHeaderModifierFilter>),
    StaticResponse(HttpStaticResponseFilterRef),
}

#[derive(Validate, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, JsonSchema)]
pub struct HttpRouteRuleKey(String);

impl<S: AsRef<str>> From<S> for HttpRouteRuleKey {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

#[derive(Validate, Getters, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteRule {
    #[getset(get = "pub")]
    key: HttpRouteRuleKey,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    matches: Vec<HttpRequestMatches>,

    #[getset(get = "pub")]
    #[validate(max_items = 16)]
    backends: Vec<HttpRouteBackend>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(max_items = 16)]
    filters: Vec<HttpRouteRuleFilter>,
}

impl HttpRouteRule {
    pub fn builder<K: Into<HttpRouteRuleKey>>(key: K) -> HttpRouteRuleBuilder {
        HttpRouteRuleBuilder {
            key: key.into(),
            match_builders: Vec::new(),
            backends: Vec::new(),
            filters: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct HttpRouteRuleBuilder {
    key: HttpRouteRuleKey,
    match_builders: Vec<HttpRouteRuleMatchesBuilder>,
    backends: Vec<HttpRouteBackend>,
    filters: Vec<HttpRouteRuleFilter>,
}

impl HttpRouteRuleBuilder {
    pub fn build(self) -> HttpRouteRule {
        HttpRouteRule {
            key: self.key,
            matches: self
                .match_builders
                .into_iter()
                .map(HttpRouteRuleMatchesBuilder::build)
                .collect(),
            backends: self.backends,
            filters: self.filters,
        }
    }

    pub fn add_match<F>(&mut self, factory: F) -> &mut Self
    where
        F: FnOnce(&mut HttpRouteRuleMatchesBuilder),
    {
        let mut builder = HttpRequestMatches::builder();
        factory(&mut builder);
        self.match_builders.push(builder);
        self
    }

    pub fn add_backend(&mut self, backend: HttpRouteBackend) -> &mut Self {
        self.backends.push(backend);
        self
    }

    pub fn add_filter(&mut self, filter: HttpRouteRuleFilter) -> &mut Self {
        self.filters.push(filter);
        self
    }
}
