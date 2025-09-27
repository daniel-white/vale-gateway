use super::Matcher;
use super::basic::{ExactMatcher, RegularExpressionMatcher};
use super::scoring::RequestMatcherScorer;
use http::request::Parts;
use regex::Regex;
use std::borrow::Cow;
use std::sync::Arc;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct QueryParamNameMatcher {
    matcher: ExactMatcher<String>,
}

impl QueryParamNameMatcher {
    fn matches(&self, name: &str) -> bool {
        self.matcher.matches_str(name)
    }
}

#[derive(Debug)]
pub enum QueryParamValueMatcher {
    Exact(ExactMatcher<String>),
    RegularExpression(RegularExpressionMatcher),
}

impl QueryParamValueMatcher {
    fn matches(&self, value: &str) -> bool {
        match self {
            QueryParamValueMatcher::Exact(matcher) => matcher.matches_str(value),
            QueryParamValueMatcher::RegularExpression(matcher) => matcher.matches(value),
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct QueryParamMatcher {
    name_matcher: QueryParamNameMatcher,
    value_matcher: QueryParamValueMatcher,
}

impl QueryParamMatcher {
    fn new(name_matcher: QueryParamNameMatcher, value_matcher: QueryParamValueMatcher) -> Self {
        Self::builder()
            .name_matcher(name_matcher)
            .value_matcher(value_matcher)
            .build()
    }

    pub fn new_exact(name: Arc<String>, value: Arc<String>) -> Self {
        let name_matcher = QueryParamNameMatcher::builder()
            .matcher(ExactMatcher::new(name))
            .build();
        let value_matcher = QueryParamValueMatcher::Exact(ExactMatcher::new(value));

        Self::new(name_matcher, value_matcher)
    }

    pub fn new_matching(name: Arc<String>, regex: Arc<Regex>) -> Self {
        let name_matcher = QueryParamNameMatcher::builder()
            .matcher(ExactMatcher::new(name))
            .build();
        let value_matcher =
            QueryParamValueMatcher::RegularExpression(RegularExpressionMatcher::new(regex));
        Self::new(name_matcher, value_matcher)
    }

    #[instrument(
        skip(self, name, value),
        name = "QueryParamMatch::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (name, value): &(Cow<str>, Cow<str>)) -> bool {
        self.name_matcher.matches(name) && self.value_matcher.matches(value)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct QueryParamsMatcher {
    matchers: Vec<QueryParamMatcher>,
}

impl QueryParamsMatcher {
    pub fn weight(&self) -> usize {
        self.matchers.len()
    }
}

impl Matcher for QueryParamsMatcher {
    #[instrument(skip(self, scorer, req), name = "QueryParamsMatcher::matches")]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let query_params: Vec<(Cow<str>, Cow<str>)> = req
            .uri
            .query()
            .map(|query| url::form_urlencoded::parse(query.as_bytes()).collect())
            .unwrap_or_default();

        if query_params.is_empty() {
            debug!("Request has no query parameters");
            return false;
        }

        let is_match = self.matchers.iter().all(|m| {
            query_params
                .iter()
                .any(|query_param| m.matches(query_param))
        });

        if is_match {
            debug!("Query parameters matched");
            scorer.query_params(self);
        }
        is_match
    }
}
