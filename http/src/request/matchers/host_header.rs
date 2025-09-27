use super::basic::{ExactDnsNameMatcher, InZoneDnsNameMatcher};
use hickory_proto::rr::Name;
use http::header::HOST;
use http::uri::Authority;
use http::{HeaderMap, HeaderValue};
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug)]
pub enum HostHeaderValueMatcher {
    Exact(ExactDnsNameMatcher),
    InZone(InZoneDnsNameMatcher),
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
    value_matchers: Vec<HostHeaderValueMatcher>,
}

impl HostHeaderMatcher {
    #[instrument(skip(self, headers), name = "HostHeaderMatcher::matches")]
    pub fn matches(&self, headers: &HeaderMap) -> bool {
        if self.value_matchers.is_empty() {
            debug!("No host header matches configured");
            return true;
        }

        let is_match = match headers.get(HOST) {
            Some(value) => self.value_matchers.iter().any(|m| m.matches(value)),
            None => false, // If there's no Host header, it doesn't match
        };

        if is_match {
            debug!("Host header matched");
        }

        is_match
    }
}
