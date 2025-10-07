use derive_more::From;
use getset::{CloneGetters, Getters};
use http::HeaderName;
use http::header::FORWARDED;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use vg_core::http::header::{X_FORWARDED_BY, X_FORWARDED_FOR, X_FORWARDED_HOST, X_FORWARDED_PROTO};
use vg_core::net::IpRef;

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters, CloneGetters, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct TrustedHeaderClientAddressExtractor {
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
pub struct TrustedProxiesClientAddressExtractor {
    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    trusted_headers: Vec<TrustedProxyHeaderName>,

    #[getset(get = "pub")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    proxies: Vec<IpRef>,
}

impl TrustedProxiesClientAddressExtractor {
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

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From)]
#[serde(tag = "extractor", rename_all = "camelCase")]
pub enum ClientAddressExtractor {
    None,
    #[default]
    Direct,
    TrustedHeader(TrustedHeaderClientAddressExtractor),
    TrustedProxies(TrustedProxiesClientAddressExtractor),
}

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TypedBuilder,
    Getters,
    CloneGetters,
)]
#[serde(rename_all = "camelCase")]
pub struct ClientAddressesPolicy {
    #[getset(get = "pub")]
    #[builder(default, setter(into))]
    #[serde(flatten)]
    extractor: ClientAddressExtractor,

    #[getset(get_clone = "pub")]
    #[builder(setter(into))]
    #[serde(
        default,
        with = "http_serde_ext::header_name::option",
        skip_serializing_if = "Option::is_none"
    )]
    backend_header: Option<HeaderName>,
}

impl ClientAddressesPolicy {
    pub fn is_default(&self) -> bool {
        self == &ClientAddressesPolicy::default() && self.backend_header.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipnet::IpNet;
    use serde_json;
    use std::net::IpAddr;

    #[test]
    fn test_serialize_client_addr_none() {
        let policy = ClientAddressesPolicy::builder()
            .extractor(ClientAddressExtractor::None)
            .backend_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            json,
            r#"{"extractor":"none","backendHeader":"x-client-ip"}"#
        );
    }

    #[test]
    fn test_serialize_client_addr_direct() {
        let policy = ClientAddressesPolicy::builder()
            .extractor(ClientAddressExtractor::Direct)
            .backend_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            json,
            r#"{"extractor":"direct","backendHeader":"x-client-ip"}"#
        );
    }

    #[test]
    fn test_serialize_client_addr_header() {
        let extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(HeaderName::from_static("x-real-ip"))
            .build();
        let policy = ClientAddressesPolicy::builder()
            .extractor(extractor)
            .backend_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            json,
            r#"{"extractor":"trustedHeader","trustedHeader":"x-real-ip","backendHeader":"x-client-ip"}"#
        );
    }

    #[test]
    fn test_serialize_client_addr_proxies() {
        let addr = IpAddr::from([192, 168, 1, 1]);
        let ip1 = IpRef::Addr(addr);
        let ip2 = IpRef::Net(IpNet::new(addr, 24).unwrap());
        let extractor = TrustedProxiesClientAddressExtractor::builder()
            .trusted_headers(vec![
                TrustedProxyHeaderName::XForwardedFor,
                TrustedProxyHeaderName::Forwarded,
            ])
            .proxies(vec![ip1, ip2])
            .build();
        let policy = ClientAddressesPolicy::builder()
            .extractor(extractor)
            .backend_header(Some(HeaderName::from_static("x-real-ip")))
            .build();
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            json,
            r#"{"extractor":"trustedProxies","trustedHeaders":["x-forwarded-for","forwarded"],"proxies":["192.168.1.1","192.168.1.1/24"],"backendHeader":"x-real-ip"}"#
        );
    }
}
