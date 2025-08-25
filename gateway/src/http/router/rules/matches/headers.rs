use super::Match;
use crate::http::router::rules::scoring::HttpRouteRuleMatchingScoreBuilder;
use crate::infra::get_regex;
use http::{HeaderMap, HeaderName, HeaderValue};
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Eq)]
pub struct HeaderNameMatch {
    header: HeaderName,
}

impl From<HeaderName> for HeaderNameMatch {
    fn from(header: HeaderName) -> Self {
        Self { header }
    }
}

impl HeaderNameMatch {
    fn matches(&self, name: &HeaderName) -> bool {
        self.header == name
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeaderValueMatch {
    Exact(HeaderValue),
    RegularExpression(String),
}

impl HeaderValueMatch {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatch::Exact(expected_value) => expected_value == value,
            HeaderValueMatch::RegularExpression(regex) => match value.to_str() {
                Ok(value) => {
                    let regex = get_regex(regex);
                    regex.is_match(value)
                }
                Err(_) => false,
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HeaderMatch {
    name_match: HeaderNameMatch,
    value_match: HeaderValueMatch,
}

impl HeaderMatch {
    pub fn new_exact<H: Into<HeaderName>, V: Into<HeaderValue>>(name: H, value: V) -> Self {
        let name: HeaderName = name.into();
        Self {
            name_match: name.into(),
            value_match: HeaderValueMatch::Exact(value.into()),
        }
    }

    pub fn new_matching<H: Into<HeaderName>>(name: H, pattern: &str) -> Self {
        let name: HeaderName = name.into();
        Self {
            name_match: name.into(),
            value_match: HeaderValueMatch::RegularExpression(pattern.to_string()),
        }
    }

    #[instrument(
        skip(self, name, value),
        name = "HeaderMatch::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (name, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name_match.matches(name) && self.value_match.matches(value)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HeadersMatch {
    header_matches: Vec<HeaderMatch>,
}

impl HeadersMatch {
    pub fn builder() -> HeadersMatchBuilder {
        HeadersMatchBuilder {
            header_matches: Vec::new(),
        }
    }
}

impl Match<HeaderMap> for HeadersMatch {
    #[instrument(skip(self, score, headers), name = "HeadersMatch::matches")]
    fn matches(&self, score: &HttpRouteRuleMatchingScoreBuilder, headers: &HeaderMap) -> bool {
        let is_match = self
            .header_matches
            .iter()
            .all(|m| headers.iter().any(|header| m.matches(&header)));
        if is_match {
            debug!("Headers matched");
            score.headers(self, self.header_matches.len());
        }
        is_match
    }
}

#[derive(Debug)]
pub struct HeadersMatchBuilder {
    header_matches: Vec<HeaderMatch>,
}

impl HeadersMatchBuilder {
    pub fn build(self) -> Option<HeadersMatch> {
        if self.header_matches.is_empty() {
            return None;
        }
        Some(HeadersMatch {
            header_matches: self.header_matches,
        })
    }

    pub fn with_exact<H: Into<HeaderName>, V: Into<HeaderValue>>(
        &mut self,
        name: H,
        value: V,
    ) -> &mut Self {
        self.header_matches
            .push(HeaderMatch::new_exact(name, value));
        self
    }

    pub fn with_matching<H: Into<HeaderName>>(&mut self, name: H, pattern: &str) -> &mut Self {
        self.header_matches
            .push(HeaderMatch::new_matching(name, pattern));
        self
    }
}
