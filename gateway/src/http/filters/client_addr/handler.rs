use super::extractors::{
    ClientAddrExtractorType, NoopClientAddrExtractor, TrustedHeaderClientAddrExtractor,
    TrustedProxiesClientAddrExtractor,
};
use super::VALE_GATEWAY_CLIENT_IP_HEADER;
use crate::http::filters::client_addr::extractors::ClientAddrExtractorType::{
    Noop, TrustedHeader, TrustedProxies,
};
use http::{request, HeaderValue};
use std::net::{IpAddr, SocketAddr};
use typed_builder::TypedBuilder;
use vg_core::http::filters::client_addr::{
    HttpClientAddrFilter, HttpClientAddrSource, HttpProxyHeaders,
};

#[derive(Debug, PartialEq, Eq, TypedBuilder)]
pub struct HttpClientAddrFilterHandler {
    extractor: ClientAddrExtractorType,
}

impl HttpClientAddrFilterHandler {
    pub fn from(filter: &HttpClientAddrFilter) -> Self {
        let extractor = match (filter.source(), filter.header(), filter.proxies()) {
            (HttpClientAddrSource::Header, Some(header), _) => {
                let extractor = TrustedHeaderClientAddrExtractor::builder()
                    .key(filter.key().clone())
                    .header(header)
                    .build();
                TrustedHeader(extractor)
            }
            (HttpClientAddrSource::Proxies, _, Some(proxies)) => {
                let mut extractor =
                    TrustedProxiesClientAddrExtractor::builder(filter.key().clone());

                for ip in proxies.trusted_ips() {
                    extractor.add_trusted_ip(*ip);
                }

                for range in proxies.trusted_ranges() {
                    extractor.add_trusted_ip_range(*range);
                }

                for header in proxies.trusted_headers() {
                    match header {
                        HttpProxyHeaders::Forwarded => {
                            extractor.trust_forwarded_header();
                        }
                        HttpProxyHeaders::XForwardedFor => {
                            extractor.trust_x_forwarded_for_header();
                        }
                        HttpProxyHeaders::XForwardedBy => {
                            extractor.trust_x_forwarded_by_header();
                        }
                        HttpProxyHeaders::XForwardedProto => {
                            extractor.trust_x_forwarded_proto_header();
                        }
                        HttpProxyHeaders::XForwardedHost => {
                            extractor.trust_x_forwarded_host_header();
                        }
                    }
                }

                let extractor = extractor.build();

                TrustedProxies(extractor)
            }
            (source, _, _) => {
                // TODO log
                let extractor = NoopClientAddrExtractor::builder()
                    .key(filter.key().clone())
                    .build();

                Noop(extractor)
            }
        };

        Self::builder().extractor(extractor).build()
    }

    pub fn filter(&self, addr: SocketAddr, req: &mut request::Parts) -> Option<IpAddr> {
        let extractor = self.extractor.extractor();
        if let Some(client_addr) = extractor.extract(addr, req) {
            let headers = &mut req.headers;
            headers.insert(
                VALE_GATEWAY_CLIENT_IP_HEADER,
                HeaderValue::from_str(&client_addr.to_string()).unwrap(),
            );
            Some(client_addr)
        } else {
            let headers = &mut req.headers;
            headers.remove(&VALE_GATEWAY_CLIENT_IP_HEADER); // **MUST** remove the header from the client if the address is not available
            None
        }
    }
}
