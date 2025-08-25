pub mod headers;
pub mod host_header;
pub mod method;
pub mod path;
pub mod query_params;

use self::headers::HeadersMatch;
use self::headers::HeadersMatchBuilder;
use self::method::MethodMatch;
use self::path::PathMatch;
use self::query_params::QueryParamsMatch;
use self::query_params::QueryParamsMatchBuilder;
use super::scoring::{HttpRouteRuleMatchingScore, HttpRouteRuleMatchingScoreBuilder};
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use std::borrow::Cow;
use tracing::{debug, instrument, trace};
use vg_core::http::matches::{
    HttpHeaderMatchKind, HttpPathMatchKind, HttpQueryParamMatchKind,
    HttpRequestMatches,
};

trait Match<T> {
    fn matches(&self, score: &HttpRouteRuleMatchingScoreBuilder, part: &T) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub struct HttpRouteRuleMatches {
    path: Option<PathMatch>,
    method: Option<MethodMatch>,
    headers: Option<HeadersMatch>,
    query_params: Option<QueryParamsMatch>,
}

impl HttpRouteRuleMatches {
    pub fn builder() -> HttpRouteRuleMatchBuilder {
        HttpRouteRuleMatchBuilder {
            path: None,
            method: None,
            headers_match_builder: HeadersMatch::builder(),
            query_params_builder: QueryParamsMatch::builder(),
        }
    }

    pub fn from(matches: &HttpRequestMatches) -> Self {
        let mut builder = Self::builder();

        let path = matches.path();
        match path.kind() {
            HttpPathMatchKind::Exact => {
                builder.with_exact_path(path.value());
            }
            HttpPathMatchKind::Prefix => {
                builder.with_path_prefix(path.value());
            }
            HttpPathMatchKind::RegularExpression => {
                builder.with_path_matching(path.value());
            }
        }

        if let Some(method) = matches.method().as_ref() {
            builder.with_method(method.into());
        }

        if let Some(headers) = matches.headers().as_ref() {
            for header in headers.iter() {
                match header.kind() {
                    HttpHeaderMatchKind::ExactValue => {
                        builder.with_exact_header(
                            header.header(),
                            HeaderValue::from_str(header.value()).unwrap(),
                        );
                    }
                    HttpHeaderMatchKind::ValueMatching => {
                        builder.with_header_matching(header.header(), header.value());
                    }
                }
            }
        }

        if let Some(query_params) = matches.query_params().as_ref() {
            for query_param in query_params.iter() {
                match query_param.kind() {
                    HttpQueryParamMatchKind::ExactValue => {
                        builder.with_exact_query_param(
                            query_param.name().as_str(),
                            query_param.value(),
                        );
                    }
                    HttpQueryParamMatchKind::ValueMatching => {
                        builder.with_query_param_matching(
                            query_param.name().as_str(),
                            query_param.value(),
                        );
                    }
                }
            }
        }

        builder.build()
    }

    #[instrument(skip(self, req), name = "HttpRouteRuleMatches::matches")]
    pub fn matches(&self, req: &Parts) -> HttpRouteRuleMatchResult {
        let score_builder = HttpRouteRuleMatchingScore::builder();
        let mut matched_path_prefix = None;

        if let Some(method_matcher) = &self.method {
            trace!("Testing method for match");
            if !method_matcher.matches(&score_builder, &req.method) {
                debug!("Method did not match");
                return HttpRouteRuleMatchResult::not_matched();
            }
        }

        if let Some(path_matcher) = &self.path {
            trace!("Testing path for match");
            let path_result = path_matcher.matches(&score_builder, &req.uri.path());
            if !path_result.matched() {
                debug!("Path did not match");
                return HttpRouteRuleMatchResult::not_matched();
            }
            matched_path_prefix = path_result.matched_prefix().clone();
        }

        if let Some(headers_matcher) = &self.headers {
            trace!("Testing headers for match");
            if !headers_matcher.matches(&score_builder, &req.headers) {
                debug!("Headers did not match");
                return HttpRouteRuleMatchResult::not_matched();
            }
        }

        if let Some(query_params_matcher) = &self.query_params {
            trace!("Testing query parameters for match");
            let query_params: Vec<(Cow<str>, Cow<str>)> = req
                .uri
                .query()
                .map(|query| url::form_urlencoded::parse(query.as_bytes()).collect())
                .unwrap_or_default();
            if !query_params_matcher.matches(&score_builder, &query_params) {
                debug!("Query parameters did not match");
                return HttpRouteRuleMatchResult::not_matched();
            }
        }

        debug!("All route rule matches succeeded");
        HttpRouteRuleMatchResult::matched(score_builder.build(), matched_path_prefix)
    }
}

/// Enhanced result that includes matched prefix context
#[derive(Debug, PartialEq, Eq)]
pub enum HttpRouteRuleMatchResult {
    Matched {
        score: HttpRouteRuleMatchingScore,
        matched_path_prefix: Option<String>,
    },
    NotMatched,
}

impl HttpRouteRuleMatchResult {
    pub fn matched(score: HttpRouteRuleMatchingScore, matched_path_prefix: Option<String>) -> Self {
        Self::Matched {
            score,
            matched_path_prefix,
        }
    }

    pub fn not_matched() -> Self {
        Self::NotMatched
    }

    /// Check if the result represents a match
    pub fn is_matched(&self) -> bool {
        matches!(self, Self::Matched { .. })
    }

    /// Get the score if matched
    pub fn score(&self) -> Option<&HttpRouteRuleMatchingScore> {
        match self {
            Self::Matched { score, .. } => Some(score),
            Self::NotMatched => None,
        }
    }

    /// Get the matched prefix if available
    pub fn matched_prefix(&self) -> Option<&String> {
        match self {
            Self::Matched {
                matched_path_prefix: matched_prefix,
                ..
            } => matched_prefix.as_ref(),
            Self::NotMatched => None,
        }
    }
}

#[derive(Debug)]
pub struct HttpRouteRuleMatchBuilder {
    path: Option<PathMatch>,
    method: Option<MethodMatch>,
    headers_match_builder: HeadersMatchBuilder,
    query_params_builder: QueryParamsMatchBuilder,
}

impl HttpRouteRuleMatchBuilder {
    pub fn build(self) -> HttpRouteRuleMatches {
        HttpRouteRuleMatches {
            path: self.path,
            method: self.method,
            headers: self.headers_match_builder.build(),
            query_params: self.query_params_builder.build(),
        }
    }

    pub fn with_exact_path(&mut self, path: &str) -> &mut Self {
        self.path = Some(PathMatch::Exact(path.to_string()));
        self
    }

    pub fn with_path_prefix(&mut self, prefix: &str) -> &mut Self {
        self.path = Some(PathMatch::Prefix(prefix.to_string()));
        self
    }

    pub fn with_path_matching(&mut self, pattern: &str) -> &mut Self {
        self.path = Some(PathMatch::RegularExpression(pattern.to_string()));
        self
    }

    pub fn with_method(&mut self, method: http::Method) -> &mut Self {
        self.method = Some(method.into());
        self
    }

    pub fn with_exact_header<H: Into<HeaderName>, V: Into<HeaderValue>>(
        &mut self,
        name: H,
        value: V,
    ) -> &mut Self {
        self.headers_match_builder.with_exact(name, value);
        self
    }

    pub fn with_header_matching<H: Into<HeaderName>>(
        &mut self,
        name: H,
        pattern: &str,
    ) -> &mut Self {
        self.headers_match_builder.with_matching(name, pattern);
        self
    }

    pub fn with_exact_query_param(&mut self, name: &str, value: &str) -> &mut Self {
        self.query_params_builder.with_exact(name, value);
        self
    }

    pub fn with_query_param_matching(&mut self, name: &str, pattern: &str) -> &mut Self {
        self.query_params_builder.with_matching(name, pattern);
        self
    }
}
