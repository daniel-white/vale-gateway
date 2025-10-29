use crate::extensions::ClientIp;
use crate::policy::client_addrs::extractors::ClientAddressExtractor;
use http::request::Parts;
use std::net::SocketAddr;

/// Utility for extracting client IP and setting it as a request extension
pub struct ClientIpExtractor {
    extractor: ClientAddressExtractor,
}

impl ClientIpExtractor {
    /// Create a new ClientIpExtractor with the given address extractor
    pub fn new(extractor: ClientAddressExtractor) -> Self {
        Self { extractor }
    }

    /// Extract client IP from the request and socket address, then set it as an extension
    pub fn extract_and_set(&self, client_addr: SocketAddr, parts: &mut Parts) {
        if let Some(ip) = self.extractor.extract(client_addr, parts) {
            parts.extensions.insert(ClientIp::new(ip));
        }
    }

    /// Extract client IP from the request and socket address
    pub fn extract(&self, client_addr: SocketAddr, parts: &Parts) -> Option<ClientIp> {
        self.extractor
            .extract(client_addr, parts)
            .map(ClientIp::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::client_addrs::extractors::TrustedHeaderClientAddressExtractor;
    use http::{HeaderValue, Request};
    use std::net::IpAddr;
    use std::str::FromStr;
    use vg_core::http::header::X_FORWARDED_FOR;

    fn create_request_parts() -> Parts {
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    #[test]
    fn test_direct_extraction() {
        let extractor = ClientIpExtractor::new(ClientAddressExtractor::Direct);
        let socket_addr = SocketAddr::from_str("192.168.1.100:8080").unwrap();
        let parts = create_request_parts();

        let client_ip = extractor.extract(socket_addr, &parts);

        assert!(client_ip.is_some());
        assert_eq!(
            client_ip.unwrap().ip(),
            IpAddr::from_str("192.168.1.100").unwrap()
        );
    }

    #[test]
    fn test_extract_and_set() {
        let extractor = ClientIpExtractor::new(ClientAddressExtractor::Direct);
        let socket_addr = SocketAddr::from_str("203.0.113.1:443").unwrap();
        let mut parts = create_request_parts();

        // Initially no ClientIp extension
        assert!(parts.extensions.get::<ClientIp>().is_none());

        // Extract and set
        extractor.extract_and_set(socket_addr, &mut parts);

        // Now ClientIp extension should be present
        let client_ip = parts.extensions.get::<ClientIp>();
        assert!(client_ip.is_some());
        assert_eq!(
            client_ip.unwrap().ip(),
            IpAddr::from_str("203.0.113.1").unwrap()
        );
    }

    #[test]
    fn test_trusted_header_extraction() {
        let header_extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();
        let extractor = ClientIpExtractor::new(header_extractor.into());

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut parts = create_request_parts();
        parts
            .headers
            .insert(X_FORWARDED_FOR, HeaderValue::from_static("203.0.113.42"));

        let client_ip = extractor.extract(socket_addr, &parts);

        assert!(client_ip.is_some());
        assert_eq!(
            client_ip.unwrap().ip(),
            IpAddr::from_str("203.0.113.42").unwrap()
        );
    }

    #[test]
    fn test_no_ip_extracted() {
        let header_extractor = TrustedHeaderClientAddressExtractor::builder()
            .trusted_header(X_FORWARDED_FOR)
            .build();
        let extractor = ClientIpExtractor::new(header_extractor.into());

        let socket_addr = SocketAddr::from_str("192.168.1.1:80").unwrap();
        let mut parts = create_request_parts();
        // No X-Forwarded-For header

        extractor.extract_and_set(socket_addr, &mut parts);

        // No ClientIp extension should be set
        assert!(parts.extensions.get::<ClientIp>().is_none());
    }
}
