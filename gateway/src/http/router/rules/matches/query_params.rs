use super::Match;
use crate::http::router::rules::scoring::HttpRouteRuleMatchingScoreBuilder;
use crate::infra::get_regex;
use std::borrow::Cow;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Eq)]
pub struct QueryParamNameMatch {
    name: String,
}

impl From<&str> for QueryParamNameMatch {
    fn from(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl QueryParamNameMatch {
    fn matches(&self, name: &str) -> bool {
        self.name == *name
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum QueryParamValueMatch {
    Exact(String),
    RegularExpression(String),
}

impl QueryParamValueMatch {
    fn matches(&self, value: &str) -> bool {
        match self {
            QueryParamValueMatch::Exact(expected_value) => expected_value == value,
            QueryParamValueMatch::RegularExpression(regex) => {
                let regex = get_regex(regex);
                regex.is_match(value)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryParamMatch {
    name_match: QueryParamNameMatch,
    value_match: QueryParamValueMatch,
}

impl QueryParamMatch {
    pub fn new_exact(name: &str, value: &str) -> Self {
        Self {
            name_match: name.into(),
            value_match: QueryParamValueMatch::Exact(value.to_string()),
        }
    }

    pub fn new_matching(name: &str, pattern: &str) -> Self {
        Self {
            name_match: name.into(),
            value_match: QueryParamValueMatch::RegularExpression(pattern.to_string()),
        }
    }

    #[instrument(
        skip(self, name, value),
        name = "QueryParamMatch::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (name, value): &(Cow<str>, Cow<str>)) -> bool {
        self.name_match.matches(name.as_ref()) && self.value_match.matches(value.as_ref())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryParamsMatch {
    query_param_matches: Vec<QueryParamMatch>,
}

impl QueryParamsMatch {
    pub fn builder() -> QueryParamsMatchBuilder {
        QueryParamsMatchBuilder {
            query_param_matches: Vec::new(),
        }
    }
}

impl Match<Vec<(Cow<'_, str>, Cow<'_, str>)>> for QueryParamsMatch {
    #[instrument(skip(self, score, query_params), name = "QueryParamsMatch::matches")]
    fn matches(
        &self,
        score: &HttpRouteRuleMatchingScoreBuilder,
        query_params: &Vec<(Cow<str>, Cow<str>)>,
    ) -> bool {
        let is_match = self.query_param_matches.iter().all(|m| {
            query_params
                .iter()
                .any(|query_param| m.matches(query_param))
        });
        if is_match {
            debug!("Query parameters matched");
            score.query_params(self, self.query_param_matches.len());
        }
        is_match
    }
}

#[derive(Debug)]
pub struct QueryParamsMatchBuilder {
    query_param_matches: Vec<QueryParamMatch>,
}

impl QueryParamsMatchBuilder {
    pub fn build(self) -> Option<QueryParamsMatch> {
        if self.query_param_matches.is_empty() {
            return None;
        }
        Some(QueryParamsMatch {
            query_param_matches: self.query_param_matches,
        })
    }

    pub fn with_exact(&mut self, name: &str, value: &str) -> &mut Self {
        self.query_param_matches
            .push(QueryParamMatch::new_exact(name, value));
        self
    }

    pub fn with_matching(&mut self, name: &str, pattern: &str) -> &mut Self {
        self.query_param_matches
            .push(QueryParamMatch::new_matching(name, pattern));
        self
    }
}
