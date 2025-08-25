use crate::http::filters::HttpFilterHandlers;
use crate::http::router::rules::backends::HttpRouteRuleBackend;
use crate::http::router::rules::filters::{
    collect_http_route_rule_filter_handlers, HttpRouteRuleFilterHandler,
};
use crate::http::router::rules::matches::{HttpRouteRuleMatchResult, HttpRouteRuleMatches};
use crate::http::router::rules::scoring::HttpRouteRuleMatchingScore;
use crate::infra::TopologyLocation;
use getset::{CloneGetters, Getters};
use http::request;
use std::cmp::Ordering;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_core::http::routes::rules::HttpRouteRule as CoreHttpRouteRule;

mod backends;
pub mod filters;
pub mod matches;
pub mod scoring;

#[derive(Debug, PartialEq, Eq, Getters)]
pub struct HttpRouteRule {
    matches: Vec<HttpRouteRuleMatches>,
    #[getset(get = "pub")]
    filters: Vec<HttpRouteRuleFilterHandler>,
    backends: Vec<HttpRouteRuleBackend>,
}

impl HttpRouteRule {
    pub async fn from(
        rule: &CoreHttpRouteRule,
        http_filter_handers: &HttpFilterHandlers,
        location: Arc<TopologyLocation>,
    ) -> Self {
        let matches = rule
            .matches()
            .iter()
            .map(HttpRouteRuleMatches::from)
            .collect();
        let filters =
            collect_http_route_rule_filter_handlers(rule.filters(), http_filter_handers).await;
        let backends = rule
            .backends()
            .iter()
            .map(|b| HttpRouteRuleBackend::from(b, location.clone()))
            .collect();
        Self {
            matches,
            filters,
            backends,
        }
    }

    pub fn matches(&self, req: &request::Parts) -> Option<HttpRouteRuleMatch> {
        self.matches
            .iter()
            .filter_map(|m| match m.matches(req) {
                HttpRouteRuleMatchResult::Matched {
                    score,
                    matched_path_prefix,
                } => {
                    let m = HttpRouteRuleMatch::builder()
                        .score(score)
                        .matched_path_prefix(matched_path_prefix)
                        .build();
                    Some(m)
                }
                HttpRouteRuleMatchResult::NotMatched => None,
            })
            .max()
    }
}

#[derive(Debug, PartialEq, Eq, Getters, CloneGetters, TypedBuilder)]
pub struct HttpRouteRuleMatch {
    #[getset(get_clone = "pub")]
    score: HttpRouteRuleMatchingScore,
    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    matched_path_prefix: Option<String>,
}

impl PartialOrd for HttpRouteRuleMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HttpRouteRuleMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.cmp(&other.score)
    }
}
