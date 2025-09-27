use getset::{CloneGetters, CopyGetters};
use hickory_proto::rr::Name;
use http::HeaderValue;
use regex::Regex;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ExactMatcher<T: PartialEq> {
    value: Arc<T>,
}

impl<T: PartialEq> ExactMatcher<T> {
    pub fn new(value: Arc<T>) -> Self {
        Self { value }
    }

    pub fn matches(&self, value: &T) -> bool {
        self.value.as_ref() == value
    }
}

impl ExactMatcher<String> {
    pub fn weight(&self) -> usize {
        self.value.len()
    }

    pub fn matches_str(&self, value: &str) -> bool {
        self.value.as_str() == value
    }
}

impl ExactMatcher<HeaderValue> {
    pub fn weight(&self) -> usize {
        self.value.len()
    }
}

#[derive(Debug, Clone, CloneGetters)]
pub struct StringPrefixMatcher {
    #[getset(get_clone = "pub")]
    prefix: Arc<String>,
}

impl StringPrefixMatcher {
    pub fn new(prefix: Arc<String>) -> Self {
        Self { prefix }
    }

    pub fn matches(&self, value: &str) -> bool {
        value.starts_with(self.prefix.as_ref())
    }

    pub fn weight(&self) -> usize {
        self.prefix.len()
    }
}

#[derive(Debug, Clone, CopyGetters)]
pub struct RegularExpressionMatcher {
    regex: Arc<Regex>,
    #[getset(get_copy = "pub")]
    weight: usize,
}

impl RegularExpressionMatcher {
    pub fn new(regex: Arc<Regex>) -> Self {
        let weight = regex.as_str().len() * 4;
        Self {
            regex: regex.clone(),
            weight,
        }
    }

    pub fn matches(&self, value: &str) -> bool {
        self.regex.is_match(value)
    }
}

#[derive(Debug, Clone)]
pub struct ExactDnsNameMatcher {
    name: ExactMatcher<Name>,
}

impl ExactDnsNameMatcher {
    pub fn new(name: Arc<Name>) -> Self {
        Self {
            name: ExactMatcher::new(name),
        }
    }

    pub fn matches(&self, value: &Name) -> bool {
        self.name.matches(value)
    }
}

#[derive(Debug, Clone)]
pub struct InZoneDnsNameMatcher {
    zone: Arc<Name>,
}

impl InZoneDnsNameMatcher {
    pub fn new(zone: Arc<Name>) -> Self {
        Self { zone }
    }

    pub fn matches(&self, value: &Name) -> bool {
        self.zone.zone_of(value)
    }
}
