use super::basic::{ExactMatcher, InZoneDnsNameMatcher};
use hickory_proto::rr::Name;
use http::header::HOST;
use http::uri::Authority;
use http::{HeaderMap, HeaderValue};
use thiserror::Error;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;
use vg_http_config::routing::rule::matcher::{
    HostHeaderMatcher as HostHeaderMatcherConfig,
    HostHeaderValueMatcher as HostHeaderValueMatcherConfig,
};

#[derive(Debug)]
pub enum HostHeaderValueMatcher {
    Exact(ExactMatcher<Name>),
    InZone(InZoneDnsNameMatcher),
}

#[derive(Debug, Error)]
pub enum HostHeaderValueMatcherConversionError {
    #[error("Invalid DNS name")]
    InvalidDnsName,
    #[error("Not fully qualified DNS name")]
    NotFullyQualifiedDnsName,
}

impl TryFrom<&HostHeaderValueMatcherConfig> for HostHeaderValueMatcher {
    type Error = HostHeaderValueMatcherConversionError;

    fn try_from(config: &HostHeaderValueMatcherConfig) -> Result<Self, Self::Error> {
        match config {
            HostHeaderValueMatcherConfig::Exact(name) => {
                let name = Name::from_utf8(name)
                    .map_err(|_| HostHeaderValueMatcherConversionError::InvalidDnsName)?;
                if !name.is_fqdn() {
                    return Err(HostHeaderValueMatcherConversionError::NotFullyQualifiedDnsName);
                }
                Ok(Self::Exact(name.into()))
            }
            HostHeaderValueMatcherConfig::InZone(zone) => {
                let zone = Name::from_utf8(zone)
                    .map_err(|_| HostHeaderValueMatcherConversionError::InvalidDnsName)?;
                Ok(Self::InZone(zone.into()))
            }
        }
    }
}

impl HostHeaderValueMatcher {
    #[instrument(
        skip(self, value),
        name = "HostHeaderValueMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, value: &HeaderValue) -> bool {
        let Ok(value) = value.to_str() else {
            debug!("Host header value is not a valid UTF-8 string");
            return false; // If the header value is not a valid UTF-8 string, it doesn't match
        };

        let Ok(authority) = value.parse::<Authority>() else {
            debug!("Host header value is not a valid authority");
            return false;
        };

        let Ok(name) = Name::from_utf8(authority.host()) else {
            debug!("Host header value is not a valid DNS name");
            return false; // If the header value is not a valid DNS name, it doesn't match
        };

        match self {
            Self::Exact(matcher) => matcher.matches(&name),
            Self::InZone(matcher) => matcher.matches(&name),
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct HostHeaderMatcher {
    matchers: Vec<HostHeaderValueMatcher>,
}

impl HostHeaderMatcher {
    #[instrument(skip(self, headers), name = "HostHeaderMatcher::matches")]
    pub fn matches(&self, headers: &HeaderMap) -> bool {
        if self.matchers.is_empty() {
            debug!("No host header matches configured");
            return true;
        }

        let is_match = match headers.get(HOST) {
            Some(value) => self.matchers.iter().any(|m| m.matches(value)),
            None => false, // If there's no Host header, it doesn't match
        };

        if is_match {
            debug!("Host header matched");
        }

        is_match
    }
}

#[derive(Debug, Error)]
pub enum HostHeaderMatcherConversionError {
    #[error("Invalid host header value matcher at index {0}: {1}")]
    InvalidMatcher(usize, HostHeaderValueMatcherConversionError),
}

impl TryFrom<&HostHeaderMatcherConfig> for HostHeaderMatcher {
    type Error = HostHeaderMatcherConversionError;

    fn try_from(value: &HostHeaderMatcherConfig) -> Result<Self, Self::Error> {
        let matchers = value
            .matchers()
            .iter()
            .enumerate()
            .map(|(idx, config)| {
                HostHeaderValueMatcher::try_from(config)
                    .map_err(|e| HostHeaderMatcherConversionError::InvalidMatcher(idx, e))
            })
            .collect::<Result<_, _>>()?;

        let matcher = Self::builder().matchers(matchers).build();

        Ok(matcher)
    }
}
