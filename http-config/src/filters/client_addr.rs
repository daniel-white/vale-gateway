use crate::IpRef;
use getset::{CloneGetters, Getters};
use http::HeaderName;
use http::header::FORWARDED;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::http::header::{X_FORWARDED_BY, X_FORWARDED_FOR, X_FORWARDED_HOST, X_FORWARDED_PROTO};

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct TrustedHeaderClientAddrExtractor {
    #[getset(get_clone = "pub")]
    #[serde(with = "http_serde_ext::header_name")]
    trusted_header: HeaderName,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TrustedProxyHeaderName {
    Forwarded,
    XForwardedFor,
    XForwardedHost,
    XForwardedProto,
    XForwardedBy,
}

impl From<TrustedProxyHeaderName> for HeaderName {
    fn from(value: TrustedProxyHeaderName) -> Self {
        match value {
            TrustedProxyHeaderName::Forwarded => FORWARDED,
            TrustedProxyHeaderName::XForwardedFor => X_FORWARDED_FOR,
            TrustedProxyHeaderName::XForwardedHost => X_FORWARDED_HOST,
            TrustedProxyHeaderName::XForwardedProto => X_FORWARDED_PROTO,
            TrustedProxyHeaderName::XForwardedBy => X_FORWARDED_BY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct TrustedProxiesClientAddrExtractor {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    trusted_headers: Vec<TrustedProxyHeaderName>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    proxies: Vec<IpRef>,
}

impl TrustedProxiesClientAddrExtractor {
    pub fn trust_forwarded_header(&self) -> bool {
        self.trusted_headers
            .contains(&TrustedProxyHeaderName::Forwarded)
    }

    pub fn trust_x_forwarded_for_header(&self) -> bool {
        self.trusted_headers
            .contains(&TrustedProxyHeaderName::XForwardedFor)
    }

    pub fn trust_x_forwarded_host_header(&self) -> bool {
        self.trusted_headers
            .contains(&TrustedProxyHeaderName::XForwardedHost)
    }

    pub fn trust_x_forwarded_proto_header(&self) -> bool {
        self.trusted_headers
            .contains(&TrustedProxyHeaderName::XForwardedProto)
    }

    pub fn trust_x_forwarded_by_header(&self) -> bool {
        self.trusted_headers
            .contains(&TrustedProxyHeaderName::XForwardedBy)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum ClientAddrExtractor {
    #[default]
    None,
    TrustedHeader(TrustedHeaderClientAddrExtractor),
    TrustedProxies(TrustedProxiesClientAddrExtractor),
}

impl From<TrustedHeaderClientAddrExtractor> for ClientAddrExtractor {
    fn from(value: TrustedHeaderClientAddrExtractor) -> Self {
        Self::TrustedHeader(value)
    }
}

impl From<TrustedProxiesClientAddrExtractor> for ClientAddrExtractor {
    fn from(value: TrustedProxiesClientAddrExtractor) -> Self {
        Self::TrustedProxies(value)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder, Getters, CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddrFilter {
    #[getset(get = "pub")]
    #[builder(setter(into))]
    #[serde(flatten)]
    extractor: ClientAddrExtractor,

    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    #[serde(
        default,
        with = "http_serde_ext::header_name::option",
        skip_serializing_if = "Option::is_none"
    )]
    upstream_header: Option<HeaderName>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipnet::IpNet;
    use serde_json;
    use std::net::IpAddr;

    #[test]
    fn test_serialize_client_addr_filter_none() {
        let filter = ClientAddrFilter::builder()
            .extractor(ClientAddrExtractor::None)
            .upstream_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(json, r#"{"upstreamHeader":"x-client-ip"}"#);
    }

    #[test]
    fn test_serialize_client_addr_filter_header() {
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(HeaderName::from_static("x-real-ip"))
            .build();
        let filter = ClientAddrFilter::builder()
            .extractor(extractor)
            .upstream_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            json,
            r#"{"trustedHeader":"x-real-ip","upstreamHeader":"x-client-ip"}"#
        );
    }

    #[test]
    fn test_serialize_client_addr_filter_proxies() {
        let addr = IpAddr::from([192, 168, 1, 1]);
        let ip1 = IpRef::Addr(addr);
        let ip2 = IpRef::Net(IpNet::new(addr, 24).unwrap());
        let extractor = TrustedProxiesClientAddrExtractor::builder()
            .trusted_headers(vec![
                TrustedProxyHeaderName::XForwardedFor,
                TrustedProxyHeaderName::Forwarded,
            ])
            .proxies(vec![ip1, ip2])
            .build();
        let filter = ClientAddrFilter::builder()
            .extractor(extractor)
            .upstream_header(Some(HeaderName::from_static("x-real-ip")))
            .build();
        let json = serde_json::to_string(&filter).unwrap();
        assert_eq!(
            json,
            r#"{"trustedHeaders":["x-forwarded-for","forwarded"],"proxies":["192.168.1.1","192.168.1.1/24"],"upstreamHeader":"x-real-ip"}"#
        );
    }
}
