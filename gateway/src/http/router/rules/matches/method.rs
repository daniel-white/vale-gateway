use super::Match;
use crate::http::router::rules::scoring::HttpRouteRuleMatchingScoreBuilder;
use http::Method;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Eq)]
pub struct MethodMatch {
    method: Method,
}

impl From<Method> for MethodMatch {
    fn from(method: Method) -> Self {
        Self { method }
    }
}

impl Match<Method> for MethodMatch {
    #[instrument(
        skip(self, score, method),
        name = "MethodMatcher::matches"
        fields(match = ?self)
    )]
    fn matches(&self, score: &HttpRouteRuleMatchingScoreBuilder, method: &Method) -> bool {
        let is_match = self.method == *method;
        if is_match {
            debug!("Method matched");
            score.method(self);
        }
        is_match
    }
}
