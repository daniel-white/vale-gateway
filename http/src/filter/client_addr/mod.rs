pub mod extractors;

use crate::filter::client_addr::extractors::{
    ClientAddrExtractor, ClientAddrExtractorConversionError,
};
use http::{HeaderName, HeaderValue, request};
use std::net::{IpAddr, SocketAddr};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::filter::client_addr::ClientAddrFilter;

#[derive(Debug, TypedBuilder)]
pub struct ClientAddrFilterHandler {
    #[builder(setter(into))]
    extractor: ClientAddrExtractor,
    backend_header: Option<HeaderName>,
}

impl ClientAddrFilterHandler {
    pub fn filter(&self, addr: SocketAddr, req: &mut request::Parts) -> Option<IpAddr> {
        if let Some(header) = &self.backend_header {
            req.headers.remove(header);
        }

        let ip_addr = self.extractor.extract(addr, req);

        if let Some(header) = &self.backend_header
            && let Some(ip_addr) = ip_addr
        {
            let header_value = HeaderValue::from_str(&ip_addr.to_string())
                .expect("Failed to convert IP to HeaderValue");
            req.headers.insert(header, header_value);
        }

        ip_addr
    }
}

#[derive(Debug, Error)]
pub enum ClientAddrFilterHandlerConversionError {
    #[error("Invalid client addr extractor: {0}")]
    InvalidExtractor(
        #[from]
        #[source]
        ClientAddrExtractorConversionError,
    ),
}

impl TryFrom<&ClientAddrFilter> for ClientAddrFilterHandler {
    type Error = ClientAddrFilterHandlerConversionError;

    fn try_from(value: &ClientAddrFilter) -> Result<Self, Self::Error> {
        let extractor: ClientAddrExtractor = value.extractor().try_into()?;

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(value.backend_header())
            .build();

        Ok(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::filter::client_addr::extractors::{
        TrustedHeaderClientAddrExtractor, TrustedProxies, TrustedProxiesClientAddrExtractor,
    };
    use http::{HeaderValue, request::Parts};
    use std::str::FromStr;
    use vg_core::http::header::X_FORWARDED_FOR;

    fn create_empty_parts() -> Parts {
        use http::Request;
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    #[test]
    fn test_client_addr_extraction_from_direct_connection() {
        // Test extracting client address from direct connection using actual ClientAddrFilterHandler
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractor::None;

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.100:12345").unwrap();
        let mut request_parts = create_empty_parts();

        let result = handler.filter(socket_addr, &mut request_parts);

        // NoopClientAddrExtractor returns None
        assert!(result.is_none());
    }

    #[test]
    fn test_client_addr_extraction_from_x_forwarded_for() {
        // Test extracting client address from X-Forwarded-For header using TrustedHeaderClientAddrExtractor
        use std::net::SocketAddr;

        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        // Should extract the IP from the header
        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
    }

    #[test]
    fn test_client_addr_extraction_from_x_real_ip() {
        // Test extracting client address from X-Real-IP header
        use std::net::SocketAddr;

        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(HeaderName::from_static("x-real-ip"))
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
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

    #[test]
    fn test_client_addr_trusted_proxy_validation() {
        // Test that proxy headers are trusted using TrustedProxiesExtractor
        use std::collections::HashSet;
        use std::net::SocketAddr;
        use vg_core::net::IpRef;

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

        let extractor = TrustedProxiesClientAddrExtractor::builder()
            .config(config)
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        // Request from trusted proxy
        let trusted_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(trusted_addr, &mut request_parts);

        // Should use the trusted-proxies library to extract client IP
        assert!(result.is_some());
    }

    #[test]
    fn test_client_addr_multiple_proxy_chain() {
        // Test handling multiple proxies in forwarded headers
        use std::collections::HashSet;
        use std::net::SocketAddr;

        let config = TrustedProxies::builder()
            .proxies(HashSet::new())
            .trust_forwarded_header(false)
            .trust_x_forwarded_for_header(true)
            .trust_x_forwarded_host_header(false)
            .trust_x_forwarded_proto_header(false)
            .trust_x_forwarded_by_header(false)
            .build();

        let extractor = TrustedProxiesClientAddrExtractor::builder()
            .config(config)
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts.headers.insert(
            X_FORWARDED_FOR,
            HeaderValue::from_static("203.0.113.1, 198.51.100.1, 192.168.1.1"),
        );

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
    }

    #[test]
    fn test_client_addr_header_precedence() {
        // Test header precedence when multiple headers are present
        use std::net::SocketAddr;

        // Test X-Real-IP takes precedence when both headers are present
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(HeaderName::from_static("x-real-ip"))
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.1"));
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("198.51.100.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("198.51.100.1").unwrap());
    }

    #[test]
    fn test_client_addr_ipv6_support() {
        // Test IPv6 client address extraction
        use std::net::SocketAddr;

        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("[::1]:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("2001:db8::1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("2001:db8::1").unwrap());
    }

    #[test]
    fn test_client_addr_invalid_header_handling() {
        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("invalid-ip"));

        let result = handler.filter(socket_addr, &mut request_parts);

        // Should return None when IP parsing fails
        assert!(result.is_none());
    }

    #[test]
    fn test_client_addr_cloudflare_headers() {
        // Test Cloudflare-specific headers (CF-Connecting-IP)
        use std::net::SocketAddr;

        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(HeaderName::from_static("cf-connecting-ip"))
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(None)
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

    #[test]
    fn test_client_addr_backend_header_injection() {
        // Test that backend header is properly set when configured
        use std::net::SocketAddr;

        let extractor = TrustedHeaderClientAddrExtractor::builder()
            .trusted_header(HeaderName::from_static("x-real-ip"))
            .build();

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("X-Real-IP", HeaderValue::from_static("203.0.113.1"));

        let result = handler.filter(socket_addr, &mut request_parts);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), IpAddr::from_str("203.0.113.1").unwrap());
        // Should also set the backend header
        assert!(request_parts.headers.get("x-client-ip").is_some());
        assert_eq!(
            request_parts.headers.get("x-client-ip").unwrap(),
            "203.0.113.1"
        );
    }

    #[test]
    fn test_client_addr_backend_header_removal() {
        // Test that backend header is removed when present
        use std::net::SocketAddr;

        let extractor = ClientAddrExtractor::None;

        let handler = ClientAddrFilterHandler::builder()
            .extractor(extractor)
            .backend_header(Some(HeaderName::from_static("x-client-ip")))
            .build();

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut request_parts = create_empty_parts();
        request_parts
            .headers
            .insert("x-client-ip", HeaderValue::from_static("existing-value"));

        let _result = handler.filter(socket_addr, &mut request_parts);

        // Should remove the existing header (since extractor returns None)
        assert!(request_parts.headers.get("x-client-ip").is_none());
    }
}
