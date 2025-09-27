use super::Matcher;
use crate::request::matchers::basic::{ExactMatcher, RegularExpressionMatcher};
use crate::request::matchers::scoring::RequestMatcherScorer;
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use regex::Regex;
use std::sync::Arc;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct HeaderNameMatcher {
    matcher: ExactMatcher<HeaderName>,
}

impl HeaderNameMatcher {
    fn matches(&self, name: &HeaderName) -> bool {
        self.matcher.matches(name)
    }
}

#[derive(Debug)]
pub enum HeaderValueMatcher {
    Exact(ExactMatcher<HeaderValue>),
    RegularExpression(RegularExpressionMatcher),
}

impl HeaderValueMatcher {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatcher::Exact(matcher) => matcher.matches(value),
            HeaderValueMatcher::RegularExpression(matcher) => match value.to_str() {
                Ok(value) => matcher.matches(value),
                Err(_) => false,
            },
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct HeaderMatcher {
    name_matcher: HeaderNameMatcher,
    value_matcher: HeaderValueMatcher,
}

impl HeaderMatcher {
    fn new(name_matcher: HeaderNameMatcher, value_matcher: HeaderValueMatcher) -> Self {
        Self::builder()
            .name_matcher(name_matcher)
            .value_matcher(value_matcher)
            .build()
    }

    pub fn new_exact(key: Arc<HeaderName>, value: Arc<HeaderValue>) -> Self {
        let name_matcher = HeaderNameMatcher::builder()
            .matcher(ExactMatcher::new(key))
            .build();
        let value_matcher = HeaderValueMatcher::Exact(ExactMatcher::new(value));
        Self::new(name_matcher, value_matcher)
    }

    pub fn new_matching(key: Arc<HeaderName>, regex: Arc<Regex>) -> Self {
        let name_matcher = HeaderNameMatcher::builder()
            .matcher(ExactMatcher::new(key))
            .build();
        let value_matcher =
            HeaderValueMatcher::RegularExpression(RegularExpressionMatcher::new(regex));
        Self::new(name_matcher, value_matcher)
    }

    #[instrument(
        skip(self, key, value),
        name = "HeaderMatcher::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (key, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name_matcher.matches(key) && self.value_matcher.matches(value)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct HeadersMatcher {
    matchers: Vec<HeaderMatcher>,
}

impl HeadersMatcher {
    pub fn weight(&self) -> usize {
        self.matchers.len()
    }
}

impl Matcher for HeadersMatcher {
    #[instrument(skip(self, scorer, req), name = "HeadersMatcher::matches")]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let is_match = self
            .matchers
            .iter()
            .all(|m| req.headers.iter().any(|header| m.matches(&header)));
        if is_match {
            debug!("Headers matched");
            scorer.headers(self);
        }
        is_match
    }
}
