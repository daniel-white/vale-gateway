mod extractors;

pub use extractors::*;

use http::{HeaderName, HeaderValue, request};
use std::net::{IpAddr, SocketAddr};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct ClientAddrFilterHandler {
    extractor: ClientAddrExtractorType,
    upstream_header: Option<HeaderName>,
}

impl ClientAddrFilterHandler {
    pub fn filter(&self, addr: SocketAddr, req: &mut request::Parts) -> Option<IpAddr> {
        let extractor = self.extractor.extractor();

        if let Some(header) = &self.upstream_header {
            req.headers.remove(header);
        }

        let ip_addr = extractor.extract(addr, req);

        if let Some(header) = &self.upstream_header
            && let Some(ip) = &ip_addr
        {
            let header_value = HeaderValue::from_str(&ip.to_string())
                .expect("Failed to convert IP to HeaderValue");
            req.headers.insert(header, header_value);
        }

        ip_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::*;
    use http::{HeaderValue, Method, Uri, request::Parts};
    use std::str::FromStr;

    fn create_empty_parts() -> Parts {
        use http::Request;
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    #[tokio::test]
    async fn test_client_addr_extraction_from_direct_connection() {
        // Test extracting client address from direct connection using actual ClientAddrFilterHandler
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::Noop(NoopClientAddrExtractor::builder().build());

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.100:12345").unwrap();
        let mut request_parts = create_empty_parts();

        let result = handler.filter(socket_addr, &mut request_parts);

        // NoopClientAddrExtractor returns None
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_client_addr_extraction_from_x_forwarded_for() {
        // Test extracting client address from X-Forwarded-For header using TrustedHeaderClientAddrExtractor
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-forwarded-for"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        // Should extract the IP from the header
        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_client_addr_extraction_from_x_real_ip() {
        // Test extracting client address from X-Real-IP header
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-real-ip"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("198.51.100.5"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("198.51.100.5").unwrap());
    }

    #[tokio::test]
    async fn test_client_addr_trusted_proxy_validation() {
        // Test that proxy headers are trusted using TrustedProxiesClientAddrExtractor
        use std::net::SocketAddr;

        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.add_trusted_ip(IpAddr::from_str("192.168.1.1").unwrap());
        builder.trust_x_forwarded_for_header();
        let trusted_extractor = builder.build();

        let extractor = ClientAddrExtractorType::TrustedProxies(trusted_extractor);

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        // Request from trusted proxy
        let trusted_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(trusted_addr, &mut request_parts);

        // Should use the trusted-proxies library to extract client IP
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_client_addr_multiple_proxy_chain() {
        // Test handling multiple proxies in forwarded headers
        use std::net::SocketAddr;

        let mut builder = TrustedProxiesClientAddrExtractor::builder();
        builder.trust_x_forwarded_for_header();
        let trusted_extractor = builder.build();

        let extractor = ClientAddrExtractorType::TrustedProxies(trusted_extractor);

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts.headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_static("203.0.113.1, 198.51.100.1, 192.168.1.1"),
        );

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_client_addr_header_precedence() {
        // Test header precedence when multiple headers are present
        use std::net::SocketAddr;

        // Test X-Real-IP takes precedence when both headers are present
        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-real-ip"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("203.0.113.1"));
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("198.51.100.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("198.51.100.1").unwrap());
    }

    #[tokio::test]
    async fn test_client_addr_ipv6_support() {
        // Test IPv6 client address extraction
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-forwarded-for"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("[::1]:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("2001:db8::1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("2001:db8::1").unwrap());
    }

    #[tokio::test]
    async fn test_client_addr_invalid_header_handling() {
        // Test handling of invalid IP addresses in headers
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-forwarded-for"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Forwarded-For", HeaderValue::from_static("invalid-ip"));

        let result = handler.filter(socket_addr, &mut request_parts);

        // Should return None when IP parsing fails
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_client_addr_cloudflare_headers() {
        // Test Cloudflare-specific headers (CF-Connecting-IP)
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("cf-connecting-ip"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("104.16.0.1:80").unwrap(); // Cloudflare IP
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("CF-Connecting-IP", HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[tokio::test]
    async fn test_client_addr_upstream_header_injection() {
        // Test that upstream header is properly set when configured
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::TrustedHeader(
            TrustedHeaderClientAddrExtractor::builder()
                .header(HeaderName::from_static("x-real-ip"))
                .build(),
        );

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
        // Should also set the upstream header
        assert!(request_parts.headers.get("x-client-ip").is_some());
        assert_eq!(
            request_parts.headers.get("x-client-ip").unwrap(),
            "203.0.113.1"
        );
    }

    #[tokio::test]
    async fn test_client_addr_upstream_header_removal() {
        // Test that upstream header is removed when present
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractorType::Noop(NoopClientAddrExtractor::builder().build());

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .upstream_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("x-client-ip", HeaderValue::from_static("existing-value"));

        let result = handler.filter(socket_addr, &mut request_parts);

        // Should remove the existing header (since extractor returns None)
        assert!(request_parts.headers.get("x-client-ip").is_none());
    }
}
