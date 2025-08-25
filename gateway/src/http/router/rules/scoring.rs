use super::matches::headers::HeadersMatch;
use super::matches::method::MethodMatch;
use super::matches::path::PathMatch;
use super::matches::query_params::QueryParamsMatch;
use std::cell::Cell;
use std::cmp::Ordering;
use tracing::instrument;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct HttpRouteRuleMatchingScore {
    path_exact: bool,
    path_length: Option<usize>,
    method: bool,
    headers_count: Option<usize>,
    query_params_count: Option<usize>,
}

impl HttpRouteRuleMatchingScore {
    pub fn builder() -> HttpRouteRuleMatchingScoreBuilder {
        HttpRouteRuleMatchingScoreBuilder {
            path_exact: Cell::new(false),
            path_length: Cell::new(None),
            method: Cell::new(false),
            headers_count: Cell::new(None),
            query_params_count: Cell::new(None),
        }
    }
}

impl PartialOrd for HttpRouteRuleMatchingScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HttpRouteRuleMatchingScore {
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

        match (self.path_length, other.path_length) {
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

        match (self.headers_count, other.headers_count) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            _ => {}
        };

        match (self.query_params_count, other.query_params_count) {
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

pub struct HttpRouteRuleMatchingScoreBuilder {
    path_exact: Cell<bool>,
    path_length: Cell<Option<usize>>,
    method: Cell<bool>,
    headers_count: Cell<Option<usize>>,
    query_params_count: Cell<Option<usize>>,
}

impl HttpRouteRuleMatchingScoreBuilder {
    pub fn build(self) -> HttpRouteRuleMatchingScore {
        HttpRouteRuleMatchingScore {
            path_exact: self.path_exact.into_inner(),
            path_length: self.path_length.into_inner(),
            method: self.method.into_inner(),
            headers_count: self.headers_count.into_inner(),
            query_params_count: self.query_params_count.into_inner(),
        }
    }

    pub fn path(&self, path_match: &PathMatch) {
        match path_match {
            PathMatch::Exact(_) => {
                self.path_exact.replace(true);
            }
            PathMatch::Prefix(prefix) => {
                self.path_length.replace(Some(prefix.len()));
            }
            PathMatch::RegularExpression(pattern) => {
                self.path_length.replace(Some(pattern.len() * 4));
            }
        };
    }

    pub fn method(&self, _method_match: &MethodMatch) {
        self.method.replace(true);
    }

    pub fn headers(&self, _headers_match: &HeadersMatch, header_params_count: usize) {
        self.headers_count.replace(Some(header_params_count));
    }

    pub fn query_params(&self, _query_params_match: &QueryParamsMatch, query_params_count: usize) {
        self.query_params_count.replace(Some(query_params_count));
    }
}
