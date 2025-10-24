use http::HeaderName;
use http::request::Parts;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::policy::client_addrs::{
    ClientAddressExtractor as ClientAddressExtractorConfig,
    TrustedHeaderClientAddressExtractor as TrustedHeaderClientAddressExtractorConfig,
    TrustedProxiesClientAddressExtractor as TrustedProxiesClientAddressExtractorConfig,
};
use vg_core::net::IpRef;

trait Extractor: Into<ClientAddressExtractor> {
    fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr>;
}

#[derive(Debug)]
pub enum ClientAddressExtractor {
    Direct,
    TrustedHeader(TrustedHeaderClientAddressExtractor),
    TrustedProxies(TrustedProxiesClientAddressExtractor),
}

impl ClientAddressExtractor {
    pub fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        match self {
            Self::Direct => Some(client_addr.ip()),
            Self::TrustedHeader(extractor) => extractor.extract(client_addr, req),
            Self::TrustedProxies(extractor) => extractor.extract(client_addr, req),
        }
    }
}

#[derive(Debug, Error)]
pub enum ClientAddressExtractorConversionError {
    #[error("Unable to convert trusted header extractor: {0}")]
    TrustedHeader(
        #[from]
        #[source]
        TrustedHeaderClientAddressExtractorConversionError,
    ),
    #[error("Unable to convert trusted header extractor: {0}")]
    TrustedProxies(
        #[from]
        #[source]
        TrustedProxiesClientAddressExtractorConversionError,
    ),
}

impl TryFrom<&ClientAddressExtractorConfig> for ClientAddressExtractor {
    type Error = ClientAddressExtractorConversionError;

    fn try_from(value: &ClientAddressExtractorConfig) -> Result<Self, Self::Error> {
        let extractor = match value {
            ClientAddressExtractorConfig::Direct => Self::Direct,
            ClientAddressExtractorConfig::TrustedHeader(config) => {
                TrustedHeaderClientAddressExtractor::try_from(config)?.into()
            }
            ClientAddressExtractorConfig::TrustedProxies(config) => {
                TrustedProxiesClientAddressExtractor::try_from(config)?.into()
            }
        };

        Ok(extractor)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct TrustedHeaderClientAddressExtractor {
    #[builder(setter(into))]
    trusted_header: HeaderName,
}

#[derive(Debug, Error)]
pub enum TrustedHeaderClientAddressExtractorConversionError {}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&TrustedHeaderClientAddressExtractorConfig> for TrustedHeaderClientAddressExtractor {
    type Error = TrustedHeaderClientAddressExtractorConversionError;

    fn try_from(value: &TrustedHeaderClientAddressExtractorConfig) -> Result<Self, Self::Error> {
        let extractor = Self::builder()
            .trusted_header(value.trusted_header())
            .build();

        Ok(extractor)
    }
}

impl Extractor for TrustedHeaderClientAddressExtractor {
    fn extract(&self, _client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        req.headers
            .get(&self.trusted_header)
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
    }
}

impl From<TrustedHeaderClientAddressExtractor> for ClientAddressExtractor {
    fn from(val: TrustedHeaderClientAddressExtractor) -> Self {
        Self::TrustedHeader(val)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct TrustedProxiesClientAddressExtractor {
    #[builder(setter(into))]
    config: trusted_proxies::Config,
}

impl Extractor for TrustedProxiesClientAddressExtractor {
    fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        let trusted_ip = trusted_proxies::Trusted::from(client_addr.ip(), req, &self.config).ip();
        Some(trusted_ip)
    }
}

impl From<TrustedProxiesClientAddressExtractor> for ClientAddressExtractor {
    fn from(val: TrustedProxiesClientAddressExtractor) -> Self {
        Self::TrustedProxies(val)
    }
}

#[derive(Debug, Error)]
pub enum TrustedProxiesClientAddressExtractorConversionError {
    #[error("No trusted proxies or headers configured")]
    NoTrustedProxiesOrHeaders,
}

impl TryFrom<&TrustedProxiesClientAddressExtractorConfig> for TrustedProxiesClientAddressExtractor {
    type Error = TrustedProxiesClientAddressExtractorConversionError;

    fn try_from(value: &TrustedProxiesClientAddressExtractorConfig) -> Result<Self, Self::Error> {
        let proxies: HashSet<IpRef> = value.proxies().iter().copied().collect();
        if proxies.is_empty() && value.trusted_headers().is_empty() {
            return Err(
                TrustedProxiesClientAddressExtractorConversionError::NoTrustedProxiesOrHeaders,
            );
        }

        let config = TrustedProxies::builder()
            .proxies(proxies)
            .trust_forwarded_header(value.trust_forwarded_header())
            .trust_x_forwarded_for_header(value.trust_x_forwarded_for_header())
            .trust_x_forwarded_host_header(value.trust_x_forwarded_host_header())
            .trust_x_forwarded_proto_header(value.trust_x_forwarded_proto_header())
            .trust_x_forwarded_by_header(value.trust_x_forwarded_by_header())
            .build();

        let extractor = Self::builder()
            .config(trusted_proxies::Config::from(config))
            .build();

        Ok(extractor)
    }
}

#[derive(Debug, TypedBuilder)]
pub struct TrustedProxies {
    proxies: HashSet<IpRef>,
    trust_forwarded_header: bool,
    trust_x_forwarded_for_header: bool,
    trust_x_forwarded_host_header: bool,
    trust_x_forwarded_proto_header: bool,
    trust_x_forwarded_by_header: bool,
}

impl From<TrustedProxies> for trusted_proxies::Config {
    fn from(value: TrustedProxies) -> Self {
        let mut config = Self::new();
        for ip in value.proxies {
            let ip: String = ip.into();
            let _ = config.add_trusted_ip(&ip);
        }
        if value.trust_forwarded_header {
            config.trust_forwarded();
        }
        if value.trust_x_forwarded_for_header {
            config.trust_x_forwarded_for();
        }
        if value.trust_x_forwarded_host_header {
            config.trust_x_forwarded_host();
        }
        if value.trust_x_forwarded_proto_header {
            config.trust_x_forwarded_proto();
        }
        if value.trust_x_forwarded_by_header {
            config.trust_x_forwarded_by();
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use assertables::{assert_none, assert_some_eq_x};
    use http::HeaderValue;
    use ipnet::IpNet;
    use std::str::FromStr;
    use vg_core::http::header::X_FORWARDED_FOR;

    fn create_empty_parts() -> Parts {
        use http::Request;
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    #[tokio::test]
    async fn test_direct_extractor() {
        // Test Direct extractor that returns the socket address IP
        let socket_addr = SocketAddr::from_str("192.168.1.100:12345").unwrap();
        let request_parts = create_empty_parts();

        let extractor = ClientAddressExtractor::Direct;
        let result = extractor.extract(socket_addr, &request_parts);

        // Direct extractor returns the socket address IP
        assert_some_eq_x!(result, IpAddr::from_str("192.168.1.100").unwrap());
    }

    #[tokio::test]
    async fn test_direct_extractor_ipv6() {
        // Test Direct extractor with IPv6 address
        let socket_addr = SocketAddr::from_str("[2001:db8::1]:8080").unwrap();
        let request_parts = create_empty_parts();

        let extractor = ClientAddressExtractor::Direct;
        let result = extractor.extract(socket_addr, &request_parts);

        // Direct extractor returns the socket address IP (IPv6)
        assert_some_eq_x!(result, IpAddr::from_str("2001:db8::1").unwrap());
    }

    #[tokio::test]
    async fn test_direct_extractor_ignores_headers() {
        // Test that Direct extractor ignores headers and always returns socket IP
        let socket_addr = SocketAddr::from_str("192.168.1.100:12345").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("198.51.100.1"));

        let extractor = ClientAddressExtractor::Direct;
        let result = extractor.extract(socket_addr, &request_parts);

        // Direct extractor always returns the socket address IP, ignoring headers
        assert_some_eq_x!(result, IpAddr::from_str("192.168.1.100").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_chain_fallback() {
        // Test TrustedProxiesExtractor with multiple trusted settings
        let mut proxies: HashSet<IpRef> = HashSet::new();
        proxies.insert(IpRef::Net(IpNet::from_str("192.168.1.1/24").unwrap()));
        let config = TrustedProxies::builder()
            .proxies(proxies)
            .trust_forwarded_header(false)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(false)
            .trust_x_forwarded_proto_header(false)
            .trust_x_forwarded_by_header(false)
            .build();

        let extractor = TrustedProxiesClientAddressExtractor::builder()
            .config(trusted_proxies::Config::from(config))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_trusted_proxy_extractor() {
        // Test TrustedProxiesExtractor with specific trusted IPs
        let config = TrustedProxies::builder()
            .proxies({
                let mut proxies = HashSet::new();
                proxies.insert(IpRef::Addr(IpAddr::from_str("192.168.1.1").unwrap()));
                proxies
            })
            .trust_forwarded_header(false)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(false)
            .trust_x_forwarded_proto_header(false)
            .trust_x_forwarded_by_header(false)
            .build();

        let extractor = TrustedProxiesClientAddressExtractor::builder()
            .config(trusted_proxies::Config::from(config))
            .build();

        // Request from trusted proxy
        let trusted_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(trusted_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_invalid_ip_handling() {
        // Test handling of invalid IP addresses in headers
        let extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("invalid-ip"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_none!(result);
    }

    #[tokio::test]
    async fn test_extractor_ipv6_support() {
        // Test IPv6 address extraction
        let extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let socket_addr = SocketAddr::from_str("[::1]:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("2001:db8::1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("2001:db8::1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_private_ip_filtering() {
        // Test that TrustedProxiesExtractor can handle trusted IP ranges
        let config = TrustedProxies::builder()
            .proxies({
                let mut proxies = HashSet::new();
                // Use IpRef::Net for network ranges
                proxies.insert(IpRef::Net(IpNet::from_str("192.168.1.0/24").unwrap()));
                proxies
            })
            .trust_forwarded_header(false)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(false)
            .trust_x_forwarded_proto_header(false)
            .trust_x_forwarded_by_header(false)
            .build();

        let extractor = TrustedProxiesClientAddressExtractor::builder()
            .config(trusted_proxies::Config::from(config))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.100:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_max_proxy_depth() {
        // Test proxy chain handling with TrustedProxiesExtractor
        let mut proxies: HashSet<IpRef> = HashSet::new();
        proxies.insert(IpRef::Net(IpNet::from_str("192.168.1.1/24").unwrap()));
        proxies.insert(IpRef::Net(IpNet::from_str("198.51.100.5/24").unwrap()));
        let config = TrustedProxies::builder()
            .proxies(proxies)
            .trust_forwarded_header(false)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(false)
            .trust_x_forwarded_proto_header(false)
            .trust_x_forwarded_by_header(false)
            .build();

        let extractor = TrustedProxiesClientAddressExtractor::builder()
            .config(trusted_proxies::Config::from(config))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts.headers.insert(
            X_FORWARDED_FOR,
            HeaderValue::from_static("203.0.113.1, 198.51.100.5, 192.168.1.100"),
        );

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_header_case_insensitive() {
        // Test that header matching is case-insensitive in TrustedHeaderClientAddressExtractor
        let extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_multiple_header_values() {
        // Test handling multiple values in the same header
        let extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        // Add multiple X-Forwarded-For headers
        request_parts
            .headers
            .append(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));
        request_parts
            .headers
            .append(X_FORWARDED_FOR, HeaderValue::from_static("198.51.100.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_trusted_proxies_builder_methods() {
        // Test the TrustedProxiesExtractor with comprehensive config
        let config = TrustedProxies::builder()
            .proxies({
                let mut proxies = HashSet::new();
                // Use IpRef::Addr for individual IPs and IpRef::Net for networks
                proxies.insert(IpRef::Addr(IpAddr::from_str("192.168.1.1").unwrap()));
                proxies.insert(IpRef::Net(IpNet::from_str("10.0.0.0/8").unwrap()));
                proxies
            })
            .trust_forwarded_header(true)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(true)
            .trust_x_forwarded_proto_header(true)
            .trust_x_forwarded_by_header(true)
            .build();

        let extractor = TrustedProxiesClientAddressExtractor::builder()
            .config(trusted_proxies::Config::from(config))
            .build();

        // Verify extractor was built successfully
        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .append(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert_some_eq_x!(result, IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_type_enum() {
        let header_extractor: ClientAddressExtractor =
            TrustedHeaderClientAddressExtractor::builder()
                .trusted_header(HeaderName::from_static("x-real-ip"))
                .build()
                .into();

        let mut proxies: HashSet<IpRef> = HashSet::new();
        proxies.insert(IpRef::Net(IpNet::from_str("192.168.1.1/24").unwrap()));

        let config = TrustedProxies::builder()
            .proxies(proxies)
            .trust_forwarded_header(false)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(false)
            .trust_x_forwarded_proto_header(false)
            .trust_x_forwarded_by_header(false)
            .build();

        let proxies_extractor: ClientAddressExtractor =
            TrustedProxiesClientAddressExtractor::builder()
                .config(trusted_proxies::Config::from(config))
                .build()
                .into();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("203.0.113.1"));
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.2"));

        // Test each extractor type
        let header_result = header_extractor.extract(socket_addr, &request_parts);
        let proxies_result = proxies_extractor.extract(socket_addr, &request_parts);
        
        assert_some_eq_x!(header_result, IpAddr::from_str("203.0.113.1").unwrap());
        assert_some_eq_x!(proxies_result, IpAddr::from_str("203.0.113.2").unwrap());
    }
    #[tokio::test]
    async fn test_direct_extractor_with_different_ports() {
        // Test Direct extractor with various port numbers
        let test_cases = vec![
            ("127.0.0.1:80", "127.0.0.1"),
            ("203.0.113.5:443", "203.0.113.5"),
            ("10.0.0.1:8080", "10.0.0.1"),
            ("192.168.1.254:65535", "192.168.1.254"),
        ];

        for (socket_str, expected_ip_str) in test_cases {
            let socket_addr = SocketAddr::from_str(socket_str).unwrap();
            let request_parts = create_empty_parts();

            let extractor = ClientAddressExtractor::Direct;
            let result = extractor.extract(socket_addr, &request_parts);

            assert_some_eq_x!(result, IpAddr::from_str(expected_ip_str).unwrap());
        }
    }

    #[tokio::test]
    async fn test_direct_extractor_with_loopback_addresses() {
        // Test Direct extractor with loopback addresses
        let ipv4_loopback = SocketAddr::from_str("127.0.0.1:3000").unwrap();
        let ipv6_loopback = SocketAddr::from_str("[::1]:3000").unwrap();
        let request_parts = create_empty_parts();

        let extractor = ClientAddressExtractor::Direct;

        // Test IPv4 loopback
        let ipv4_result = extractor.extract(ipv4_loopback, &request_parts);
        assert_some_eq_x!(ipv4_result, IpAddr::from_str("127.0.0.1").unwrap());

        // Test IPv6 loopback
        let ipv6_result = extractor.extract(ipv6_loopback, &request_parts);
        assert_some_eq_x!(ipv6_result, IpAddr::from_str("::1").unwrap());
    }

    #[tokio::test]
    async fn test_direct_extractor_with_private_addresses() {
        // Test Direct extractor with common private IP address ranges
        let test_cases = vec![
            ("10.0.0.1:80", "10.0.0.1"),       // Class A private
            ("172.16.0.1:80", "172.16.0.1"),   // Class B private
            ("192.168.1.1:80", "192.168.1.1"), // Class C private
            ("169.254.1.1:80", "169.254.1.1"), // Link-local
        ];

        for (socket_str, expected_ip_str) in test_cases {
            let socket_addr = SocketAddr::from_str(socket_str).unwrap();
            let request_parts = create_empty_parts();

            let extractor = ClientAddressExtractor::Direct;
            let result = extractor.extract(socket_addr, &request_parts);

            assert_some_eq_x!(result, IpAddr::from_str(expected_ip_str).unwrap());
        }
    }

    #[tokio::test]
    async fn test_direct_extractor_consistency() {
        // Test that Direct extractor returns consistent results across multiple calls
        let socket_addr = SocketAddr::from_str("198.51.100.42:9000").unwrap();
        let request_parts = create_empty_parts();

        let extractor = ClientAddressExtractor::Direct;
        let expected_ip = IpAddr::from_str("198.51.100.42").unwrap();

        // Call extract multiple times and verify consistent results
        for _ in 0..5 {
            let result = extractor.extract(socket_addr, &request_parts);
            assert_some_eq_x!(result, expected_ip);
        }
    }
}
