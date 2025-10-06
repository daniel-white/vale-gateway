use hickory_proto::rr::Name;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostMatcher {
    Exact(String),
    InZone(String),
}

#[derive(Debug, Error)]
pub enum HostMatcherConversionError {
    #[error("Invalid DNS name")]
    InvalidDnsName,
    #[error("Not fully qualified DNS name")]
    NotFullyQualifiedDnsName,
}

impl FromStr for HostMatcher {
    type Err = HostMatcherConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = if s.starts_with("*.") {
            let s = s.trim_start_matches("*.");
            let name =
                Name::from_utf8(s).map_err(|_| HostMatcherConversionError::InvalidDnsName)?;
            HostMatcher::InZone(name.to_string())
        } else {
            let name =
                Name::from_utf8(s).map_err(|_| HostMatcherConversionError::InvalidDnsName)?;
            if !name.is_fqdn() {
                return Err(HostMatcherConversionError::NotFullyQualifiedDnsName);
            }
            HostMatcher::Exact(name.to_string())
        };

        Ok(value)
    }
}
