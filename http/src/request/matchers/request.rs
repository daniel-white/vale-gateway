use crate::request::matchers::RequestMatchDetails;
use crate::request::matchers::scoring::{RequestMatchScore, RequestMatcherScorer};
use crate::request::matchers::{Matcher, RequestMatcher};
use http::request::Parts;
use std::sync::Arc;
use tracing::{debug, instrument, trace};

impl RequestMatcher {
    #[instrument(skip(self, req), name = "RequestMatcher::matches")]
    pub fn matches(&self, req: &Parts) -> RequestMatcherResult {
        let scorer = RequestMatcherScorer::default();

        if let Some(method_matcher) = &self.method_matcher {
            trace!("Testing method for match");
            if !method_matcher.matches(&scorer, req) {
                debug!("Method did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        if let Some(path_matcher) = &self.path_matcher {
            trace!("Testing path for match");
            if !path_matcher.matches(&scorer, req) {
                debug!("Path did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        if let Some(headers_matcher) = &self.headers_matcher {
            trace!("Testing headers for match");
            if !headers_matcher.matches(&scorer, req) {
                debug!("Headers did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        if let Some(query_params_matcher) = &self.query_params_matcher {
            trace!("Testing query parameters for match");
            if !query_params_matcher.matches(&scorer, req) {
                debug!("Query parameters did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        debug!("All route rule matches succeeded");
        let score = scorer.results();
        RequestMatcherResult::Matched(score)
    }
}

/// Enhanced result that includes matched prefix context
#[derive(Debug, PartialEq, Eq)]
pub enum RequestMatcherResult {
    Matched(RequestMatchScore),
    NotMatched,
}

impl RequestMatcherResult {
    pub fn is_matched(&self) -> bool {
        matches!(self, Self::Matched { .. })
    }

    pub fn score(&self) -> Option<&RequestMatchScore> {
        match self {
            Self::Matched(score) => Some(score),
            Self::NotMatched => None,
        }
    }
}

impl RequestMatchDetails for RequestMatcherResult {
    fn path_prefix(&self) -> Option<Arc<String>> {
        self.score().and_then(|s| s.path_prefix())
    }
}
