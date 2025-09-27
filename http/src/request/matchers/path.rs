use super::Matcher;
use super::basic::{ExactMatcher, RegularExpressionMatcher, StringPrefixMatcher};
use super::scoring::RequestMatcherScorer;
use http::request::Parts;
use tracing::{debug, instrument};

#[derive(Debug)]
pub enum PathMatcher {
    Exact(ExactMatcher<String>),
    Prefix(StringPrefixMatcher),
    RegularExpression(RegularExpressionMatcher),
}

impl Matcher for PathMatcher {
    #[instrument(
        skip(self, scorer, req),
        name = "PathMatch::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let path = req.uri.path();

        let matched = match self {
            PathMatcher::Exact(matcher) => matcher.matches_str(path),
            PathMatcher::Prefix(matcher) => matcher.matches(path),
            PathMatcher::RegularExpression(matcher) => matcher.matches(path),
        };

        if matched {
            debug!("Path matched");
            scorer.path(self);
        }

        matched
    }
}
