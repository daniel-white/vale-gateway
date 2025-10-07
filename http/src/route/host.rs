use crate::route::rule::matcher::basic::{ExactMatcher, InZoneDnsNameMatcher};
use hickory_proto::rr::Name;
use http::header::HOST;
use http::uri::Authority;
use http::{HeaderMap, HeaderValue};
use thiserror::Error;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;
use vg_config::http::route::host::HostMatcher as HostMatcherConfig;

#[derive(Debug)]
pub enum HostMatcher {
    Exact(ExactMatcher<Name>),
    InZone(InZoneDnsNameMatcher),
}

#[derive(Debug, Error)]
pub enum HostMatcherConversionError {
    #[error("Invalid DNS name")]
    InvalidDnsName,
    #[error("Not fully qualified DNS name")]
    NotFullyQualifiedDnsName,
}

impl TryFrom<&HostMatcherConfig> for HostMatcher {
    type Error = HostMatcherConversionError;

    fn try_from(config: &HostMatcherConfig) -> Result<Self, Self::Error> {
        match config {
            HostMatcherConfig::Exact(name) => {
                let name = Name::from_utf8(name)
                    .map_err(|_| HostMatcherConversionError::InvalidDnsName)?;
                if !name.is_fqdn() {
                    return Err(HostMatcherConversionError::NotFullyQualifiedDnsName);
                }
                Ok(Self::Exact(name.into()))
            }
            HostMatcherConfig::InZone(zone) => {
                let zone = Name::from_utf8(zone)
                    .map_err(|_| HostMatcherConversionError::InvalidDnsName)?;
                Ok(Self::InZone(zone.into()))
            }
        }
    }
}

impl HostMatcher {
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
pub struct HostMatchers {
    matchers: Vec<HostMatcher>,
}

impl HostMatchers {
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
