use super::rules::matches::host_header::HostHeaderMatch;
use crate::http::filters::{HttpFilterHandlers, HttpRequestMatchContext};
use crate::http::router::rules::scoring::HttpRouteRuleMatchingScore;
use crate::http::router::rules::HttpRouteRule;
use crate::infra::TopologyLocation;
use futures::{stream, StreamExt};
use getset::{CloneGetters, Getters};
use http::request;
use std::cmp::Ordering;
use std::sync::Arc;
use typed_builder::TypedBuilder;
use vg_core::http::routes::{HttpRoute as CoreHttpRoute, HttpRouteKey};

#[derive(Debug, PartialEq, Eq, Getters)]
pub struct HttpRoute {
    #[getset(get = "pub")]
    key: HttpRouteKey,
    host_header_match: HostHeaderMatch,
    rules: Vec<Arc<HttpRouteRule>>,
}

impl HttpRoute {
    pub async fn from(
        route: &CoreHttpRoute,
        http_filter_handers: &HttpFilterHandlers,
        location: Arc<TopologyLocation>,
    ) -> Self {
        let rules = stream::iter(route.rules())
            .then(|r| async { HttpRouteRule::from(r, http_filter_handers, location.clone()).await })
            .map(Arc::new)
            .collect()
            .await;

        let host_header_match = HostHeaderMatch::from(route.host_header_matches());

        Self {
            key: route.key().clone(),
            host_header_match,
            rules,
        }
    }

    pub fn matches(&self, req: &request::Parts) -> Option<HttpRouteMatch> {
        self.rules
            .iter()
            .filter_map(|r| {
                r.matches(req).map(|m| (r.clone(), m))
            })
            .max_by_key(|(_, m)| m.score())
            .map(|(r, m)| {
                HttpRouteMatch::builder()
                    .rule(r)
                    .score(m.score())
                    .matched_path_prefix(m.matched_path_prefix())
                    .build()
            })
    }
}

#[derive(Debug, PartialEq, Eq, Getters, CloneGetters, TypedBuilder)]
pub struct HttpRouteMatch {
    #[getset(get_clone = "pub")]
    rule: Arc<HttpRouteRule>,
    #[getset(get_clone = "pub")]
    score: HttpRouteRuleMatchingScore,
    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    matched_path_prefix: Option<String>,
}

impl PartialOrd for HttpRouteMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HttpRouteMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.cmp(&other.score)
    }
}

impl HttpRequestMatchContext for HttpRouteMatch {
    fn matched_path_prefix(&self) -> Option<&String> {
        self.matched_path_prefix.as_ref()
    }
}
