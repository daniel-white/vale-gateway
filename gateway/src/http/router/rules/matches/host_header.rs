use hickory_proto::rr::Name;
use http::header::HOST;
use http::uri::Authority;
use http::{HeaderMap, HeaderValue};
use tracing::{debug, instrument};
use vg_core::http::matches::host_header::HttpHostHeaderMatch;
use vg_core::http::matches::HttpHostHeaderMatchKind;

#[derive(Debug, PartialEq, Eq)]
pub enum HostHeaderValueMatch {
    FullyQualified(Name),
    InZone(Name),
}

impl HostHeaderValueMatch {
    #[instrument(
        skip(self, host_header_value),
        name = "HostHeaderValueMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, host_header_value: &HeaderValue) -> bool {
        let Ok(host_header_value) = host_header_value.to_str() else {
            debug!("Host header value is not a valid UTF-8 string");
            return false; // If the header value is not a valid UTF-8 string, it doesn't match
        };

        let Ok(authority) = host_header_value.parse::<Authority>() else {
            debug!("Host header value is not a valid authority");
            return false;
        };

        let Ok(name) = Name::from_utf8(authority.host()) else {
            debug!("Host header value is not a valid DNS name");
            return false; // If the header value is not a valid DNS name, it doesn't match
        };

        match self {
            Self::FullyQualified(expected_fqdn) => expected_fqdn == &name,
            Self::InZone(expected_zone) => expected_zone.zone_of(&name),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct HostHeaderMatch {
    host_header_value_matches: Vec<HostHeaderValueMatch>,
}

impl HostHeaderMatch {
    pub fn builder() -> HostHeaderMatchBuilder {
        HostHeaderMatchBuilder::new()
    }

    pub fn from(m: &[HttpHostHeaderMatch]) -> Self {
        let mut builder = Self::builder();

        for m in m.iter() {
            match m.kind() {
                HttpHostHeaderMatchKind::FullyQualified => {
                    builder.add_fully_qualified(m.name());
                }
                HttpHostHeaderMatchKind::InZone => {
                    builder.add_in_zone(m.name());
                }
            }
        }

        builder.build()
    }

    #[instrument(skip(self, headers), name = "HostHeaderMatch::matches")]
    pub fn matches(&self, headers: &HeaderMap) -> bool {
        if self.host_header_value_matches.is_empty() {
            debug!("No host header matches configured");
            return true;
        }

        let is_match = match headers.get(HOST) {
            Some(host_header_value) => self
                .host_header_value_matches
                .iter()
                .any(|m| m.matches(host_header_value)),
            None => false, // If there's no Host header, it doesn't match
        };

        if is_match {
            debug!("Host header matched");
        }

        is_match
    }
}

pub struct HostHeaderMatchBuilder {
    host_header_value_matches: Vec<HostHeaderValueMatch>,
}

impl HostHeaderMatchBuilder {
    fn new() -> Self {
        Self {
            host_header_value_matches: Vec::new(),
        }
    }

    pub fn build(self) -> HostHeaderMatch {
        HostHeaderMatch {
            host_header_value_matches: self.host_header_value_matches,
        }
    }

    pub fn add_fully_qualified(&mut self, name: &Name) {
        let mut name = name.clone();
        name.set_fqdn(true);
        let host_header_value_match = HostHeaderValueMatch::FullyQualified(name);
        self.host_header_value_matches.push(host_header_value_match);
    }

    pub fn add_in_zone(&mut self, zone: &Name) {
        let mut name = zone.clone();
        name.set_fqdn(false);
        let host_header_value_match = HostHeaderValueMatch::InZone(name);
        self.host_header_value_matches.push(host_header_value_match);
    }
}
