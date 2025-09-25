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
