use super::Matcher;
use super::basic::ExactMatcher;
use super::scoring::RequestMatcherScorer;
use http::Method;
use http::request::Parts;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct MethodMatcher {
    method_matcher: ExactMatcher<Method>,
}

impl Matcher for MethodMatcher {
    #[instrument(
        skip(self, scorer, req),
        name = "MethodMatcher::matches"
        fields(match = ?self)
    )]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let is_match = self.method_matcher.matches(&req.method);
        if is_match {
            debug!("Method matched");
            scorer.method(self);
        }
        is_match
    }
}
