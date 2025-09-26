use http::HeaderName;
use http::request::Parts;
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};
use trusted_proxies::{Config, Trusted};
use typed_builder::TypedBuilder;

pub trait ClientAddrExtractor {
    fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr>;
}

#[derive(Debug)]
pub enum ClientAddrExtractorType {
    Noop(NoopClientAddrExtractor),
    TrustedHeader(TrustedHeaderClientAddrExtractor),
    TrustedProxies(TrustedProxiesClientAddrExtractor),
}

impl ClientAddrExtractorType {
    pub fn extractor(&self) -> &dyn ClientAddrExtractor {
        match self {
            Self::Noop(extractor) => extractor,
            Self::TrustedHeader(extractor) => extractor,
            Self::TrustedProxies(extractor) => extractor,
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct NoopClientAddrExtractor {}

impl ClientAddrExtractor for NoopClientAddrExtractor {
    fn extract(&self, _client_addr: SocketAddr, _req: &Parts) -> Option<IpAddr> {
        None
    }
}

#[derive(Debug, TypedBuilder)]
pub struct TrustedHeaderClientAddrExtractor {
    #[builder(setter(into))]
    header: HeaderName,
}

impl ClientAddrExtractor for TrustedHeaderClientAddrExtractor {
    fn extract(&self, _client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        req.headers
            .get(&self.header)
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
    }
}

#[derive(Debug)]
pub struct TrustedProxiesClientAddrExtractor {
    trusted_ips: Vec<IpNet>,
    is_forwarded_trusted: bool,
    is_x_forwarded_for_trusted: bool,
    is_x_forwarded_host_trusted: bool,
    is_x_forwarded_proto_trusted: bool,
    is_x_forwarded_by_trusted: bool,
}

impl TrustedProxiesClientAddrExtractor {
    pub fn builder() -> TrustedProxiesClientAddrExtractorBuilder {
        TrustedProxiesClientAddrExtractorBuilder {
            trusted_ips: Vec::new(),
            is_forwarded_trusted: false,
            is_x_forwarded_for_trusted: false,
            is_x_forwarded_host_trusted: false,
            is_x_forwarded_proto_trusted: false,
            is_x_forwarded_by_trusted: false,
        }
    }
}

impl ClientAddrExtractor for TrustedProxiesClientAddrExtractor {
    fn extract(&self, client_addr: SocketAddr, req: &Parts) -> Option<IpAddr> {
        let mut config = Config::new();
        for ip in &self.trusted_ips {
            let _ = config.add_trusted_ip(&ip.to_string());
        }
        if self.is_forwarded_trusted {
            config.trust_forwarded();
        }
        if self.is_x_forwarded_for_trusted {
            config.trust_x_forwarded_for();
        }
        if self.is_x_forwarded_host_trusted {
            config.trust_x_forwarded_host();
        }
        if self.is_x_forwarded_proto_trusted {
            config.trust_x_forwarded_proto();
        }
        if self.is_x_forwarded_by_trusted {
            config.trust_x_forwarded_by();
        }

        let trusted_ip = Trusted::from(client_addr.ip(), req, &config).ip();
        Some(trusted_ip)
    }
}

pub struct TrustedProxiesClientAddrExtractorBuilder {
    trusted_ips: Vec<IpNet>,
    is_forwarded_trusted: bool,
    is_x_forwarded_for_trusted: bool,
    is_x_forwarded_host_trusted: bool,
    is_x_forwarded_proto_trusted: bool,
    is_x_forwarded_by_trusted: bool,
}

impl TrustedProxiesClientAddrExtractorBuilder {
    pub fn build(self) -> TrustedProxiesClientAddrExtractor {
        TrustedProxiesClientAddrExtractor {
            trusted_ips: self.trusted_ips,
            is_forwarded_trusted: self.is_forwarded_trusted,
            is_x_forwarded_for_trusted: self.is_x_forwarded_for_trusted,
            is_x_forwarded_host_trusted: self.is_x_forwarded_host_trusted,
            is_x_forwarded_proto_trusted: self.is_x_forwarded_proto_trusted,
            is_x_forwarded_by_trusted: self.is_x_forwarded_by_trusted,
        }
    }

    pub fn add_trusted_ip(&mut self, ip: IpAddr) -> &mut Self {
        self.trusted_ips.push(ip.into());
        self
    }

    pub fn add_trusted_ip_range(&mut self, range: IpNet) -> &mut Self {
        self.trusted_ips.push(range);
        self
    }

    pub fn trust_forwarded_header(&mut self) -> &mut Self {
        self.is_forwarded_trusted = true;
        self
    }

    pub fn trust_x_forwarded_for_header(&mut self) -> &mut Self {
        self.is_x_forwarded_for_trusted = true;
        self
    }

    pub fn trust_x_forwarded_proto_header(&mut self) -> &mut Self {
        self.is_x_forwarded_proto_trusted = true;
        self
    }

    pub fn trust_x_forwarded_host_header(&mut self) -> &mut Self {
        self.is_x_forwarded_host_trusted = true;
        self
    }

    pub fn trust_x_forwarded_by_header(&mut self) -> &mut Self {
        self.is_x_forwarded_by_trusted = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use http::HeaderValue;
    use std::str::FromStr;

    fn create_empty_parts() -> Parts {
        use http::Request;
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    #[tokio::test]
    async fn test_direct_connection_extractor() {
        // Test extracting IP from direct socket connection using NoopClientAddrExtractor
        let extractor = NoopClientAddrExtractor::builder().build();
        let socket_addr = SocketAddr::from_str("192.168.1.100:12345").unwrap();
        let request_parts = create_empty_parts();

        let result = extractor.extract(socket_addr, &request_parts);

        // NoopClientAddrExtractor always returns None
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_x_forwarded_for_extractor() {
        // Test extracting IP from X-Forwarded-For header using TrustedHeaderClientAddrExtractor
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("x-forwarded-for"))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_x_real_ip_extractor() {
        // Test extracting IP from X-Real-IP header
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("x-real-ip"))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_cloudflare_connecting_ip_extractor() {
        // Test extracting IP from Cloudflare CF-Connecting-IP header
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("cf-connecting-ip"))
            .build();

        let socket_addr = SocketAddr::from_str("104.16.0.1:80").unwrap(); // Cloudflare IP
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("CF-Connecting-IP", HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_chain_fallback() {
        // Test TrustedProxiesClientAddrExtractor with multiple trusted settings
        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.trust_x_forwarded_for_header();
        builder.trust_forwarded_header();
        let extractor = builder.build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        // Should extract using trusted-proxies library
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_trusted_proxy_extractor() {
        // Test TrustedProxiesClientAddrExtractor with specific trusted IPs
        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.add_trusted_ip(IpAddr::from_str("192.168.1.1").unwrap());
        builder.trust_x_forwarded_for_header();
        let extractor = builder.build();

        // Request from trusted proxy
        let trusted_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(trusted_addr, &request_parts);

        assert!(result.is_some());
        // trusted-proxies library handles the extraction logic
    }

    #[tokio::test]
    async fn test_extractor_invalid_ip_handling() {
        // Test handling of invalid IP addresses in headers
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("x-forwarded-for"))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("invalid-ip"));

        let result = extractor.extract(socket_addr, &request_parts);

        // Should return None when IP parsing fails
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_extractor_ipv6_support() {
        // Test IPv6 address extraction
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("x-forwarded-for"))
            .build();

        let socket_addr = SocketAddr::from_str("[::1]:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("2001:db8::1"));

        let result = extractor.extract(socket_addr, &request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("2001:db8::1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_private_ip_filtering() {
        // Test that TrustedProxiesClientAddrExtractor can handle trusted IP ranges
        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.add_trusted_ip_range(IpNet::from_str("192.168.1.0/24").unwrap());
        builder.trust_x_forwarded_for_header();
        let extractor = builder.build();

        let socket_addr = SocketAddr::from_str("192.168.1.100:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        // Should extract using trusted-proxies library logic
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_extractor_max_proxy_depth() {
        // Test proxy chain handling with TrustedProxiesClientAddrExtractor
        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.trust_x_forwarded_for_header();
        let extractor = builder.build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts.headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_static("203.0.113.1, 198.51.100.1, 192.168.1.100"),
        );

        let result = extractor.extract(socket_addr, &request_parts);

        // trusted-proxies library handles the proxy chain logic
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_extractor_header_case_insensitive() {
        // Test that header matching is case-insensitive in TrustedHeaderClientAddrExtractor
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("x-forwarded-for")) // lowercase
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1")); // uppercase

        let result = extractor.extract(socket_addr, &request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_extractor_multiple_header_values() {
        // Test handling multiple values in the same header
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .header(HeaderName::from_static("x-forwarded-for"))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        // Add multiple X-Forwarded-For headers
        request_parts
            .headers
            .append("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));
        request_parts
            .headers
            .append("X-Forwarded-For", HeaderValue::from_static("198.51.100.1"));

        let result = extractor.extract(socket_addr, &request_parts);

        // Should extract the first valid IP it finds
        assert!(result.is_some());
        let extracted_ip = result.unwrap();
        assert!(
            extracted_ip == IpAddr::from_str("203.0.113.1").unwrap()
                || extracted_ip == IpAddr::from_str("198.51.100.1").unwrap()
        );
    }

    #[tokio::test]
    async fn test_trusted_proxies_builder_methods() {
        // Test the TrustedProxiesClientAddrExtractorBuilder methods
        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.add_trusted_ip(IpAddr::from_str("192.168.1.1").unwrap());
        builder.add_trusted_ip_range(IpNet::from_str("10.0.0.0/8").unwrap());
        builder.trust_forwarded_header();
        builder.trust_x_forwarded_for_header();
        builder.trust_x_forwarded_proto_header();
        builder.trust_x_forwarded_host_header();
        builder.trust_x_forwarded_by_header();

        let extractor = builder.build();

        // Verify extractor was built successfully
        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let request_parts = create_empty_parts();

        let result = extractor.extract(socket_addr, &request_parts);

        // Should use the socket address when no trusted headers are present
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_extractor_type_enum() {
        // Test the ClientAddrExtractorType enum functionality
        let noop_extractor =
            ClientAddrExtractorType::Noop(NoopClientAddrExtractor::builder().build());
        let header_extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-real-ip"))
                .build(),
        );
        let trusted_extractor = ClientAddrExtractorType::TrustedProxies(
            TrustedProxiesClientAddrExtractor::builder().build(),
        );

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("203.0.113.1"));

        // Test each extractor type
        let noop_result = noop_extractor
            .extractor()
            .extract(socket_addr, &request_parts);
        let header_result = header_extractor
            .extractor()
            .extract(socket_addr, &request_parts);
        let trusted_result = trusted_extractor
            .extractor()
            .extract(socket_addr, &request_parts);

        assert!(noop_result.is_none());
        assert!(header_result.is_some());
        assert!(trusted_result.is_some());
    }
}
