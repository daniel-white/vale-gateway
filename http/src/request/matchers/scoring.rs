use crate::request::matchers::RequestMatchDetails;
use crate::request::matchers::headers::HeadersMatcher;
use crate::request::matchers::method::MethodMatcher;
use crate::request::matchers::path::PathMatcher;
use crate::request::matchers::query_params::QueryParamsMatcher;
use std::cell::Cell;
use std::cmp::Ordering;
use std::sync::Arc;
use tracing::instrument;
use typed_builder::TypedBuilder;

#[derive(PartialEq, Eq, Debug, TypedBuilder)]
pub struct RequestMatchScore {
    path_exact: bool,
    path_weight: Option<usize>,
    path_prefix: Option<Arc<String>>, // Not for scoring, just for info
    method: bool,
    headers_weight: Option<usize>,
    query_params_weight: Option<usize>,
}

impl RequestMatchDetails for RequestMatchScore {
    fn path_prefix(&self) -> Option<Arc<String>> {
        self.path_prefix.clone()
    }
}

impl PartialOrd for RequestMatchScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RequestMatchScore {
    #[instrument(skip(self, other), name = "Score::cmp")]
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            return Ordering::Equal;
        }

        match (self.path_exact, other.path_exact) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        };

        match (self.path_weight, other.path_weight) {
            (Some(len1), Some(len2)) => match len1.cmp(&len2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        match (self.method, other.method) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        };

        match (self.headers_weight, other.headers_weight) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        match (self.query_params_weight, other.query_params_weight) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        Ordering::Equal
    }
}

#[derive(Default)]
pub struct RequestMatcherScorer {
    path_exact: Cell<bool>,
    path_weight: Cell<Option<usize>>,
    path_prefix: Cell<Option<Arc<String>>>,
    method: Cell<bool>,
    headers_weight: Cell<Option<usize>>,
    query_params_weight: Cell<Option<usize>>,
}

impl RequestMatcherScorer {
    pub fn results(self) -> RequestMatchScore {
        RequestMatchScore::builder()
            .method(self.method.into_inner())
            .headers_weight(self.headers_weight.into_inner())
            .path_exact(self.path_exact.into_inner())
            .path_prefix(self.path_prefix.into_inner())
            .path_weight(self.path_weight.into_inner())
            .query_params_weight(self.query_params_weight.into_inner())
            .build()
    }

    pub fn path(&self, path_match: &PathMatcher) {
        match path_match {
            PathMatcher::Exact(_) => {
                self.path_exact.replace(true);
            }
            PathMatcher::Prefix(matcher) => {
                let prefix = matcher.prefix();
                let weight = matcher.weight();
                self.path_prefix.replace(Some(prefix));
                self.path_weight.replace(Some(weight));
            }
            PathMatcher::RegularExpression(matcher) => {
                let weight = matcher.weight();
                self.path_weight.replace(Some(weight));
            }
        };
    }

    pub fn method(&self, _matcher: &MethodMatcher) {
        self.method.replace(true);
    }

    pub fn headers(&self, matcher: &HeadersMatcher) {
        let weight = matcher.weight();
        self.headers_weight.replace(Some(weight));
    }

    pub fn query_params(&self, matcher: &QueryParamsMatcher) {
        let weight = matcher.weight();
        self.query_params_weight.replace(Some(weight));
    }
}
