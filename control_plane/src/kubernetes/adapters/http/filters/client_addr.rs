use http::HeaderName;
use std::str::FromStr;
use tracing::warn;
use vg_api::v1alpha1::{ClientAddressesSource, GatewayConfiguration, ProxyIpAddressHeaders};
use vg_core::http::filters::client_addr::{
    HttpClientAddrFilter, HttpClientAddrFilterKey, HttpClientAddrFilterRef, HttpProxyHeaders,
};

pub fn convert_client_addrs(
    key: &str,
    gateway_configuration: &GatewayConfiguration,
) -> Option<(HttpClientAddrFilterRef, HttpClientAddrFilter)> {
    let key = HttpClientAddrFilterKey::from(key);
    let ref_ = HttpClientAddrFilterRef::builder().key(key.clone()).build();

    let client_addresses = gateway_configuration.client_addresses.as_ref()?;
    let filter = match client_addresses.source {
        ClientAddressesSource::None => HttpClientAddrFilter::builder().key(key).build(),
        ClientAddressesSource::Header => match &client_addresses
            .header
            .as_deref()
            .and_then(|h| HeaderName::from_str(h).ok())
        {
            Some(header) => HttpClientAddrFilter::builder()
                .key(key)
                .trust_header(header)
                .build(),
            None => {
                warn!("ClientAddressesSource::Header requires a valid header to be set");
                return None;
            }
        },
        ClientAddressesSource::Proxies => HttpClientAddrFilter::builder()
            .key(key)
            .trust_proxies(|p| {
                let Some(proxies) = &client_addresses.proxies else {
                    warn!("ClientAddressesSource::Proxies requires proxies to be set");
                    return;
                };
                if proxies.trust_local_ranges {
                    p.trust_local_ranges();
                }
                for trusted_ip in &proxies.trusted_ips {
                    p.add_trusted_ip(*trusted_ip);
                }
                for trusted_range in &proxies.trusted_ranges {
                    p.add_trusted_range(*trusted_range);
                }
                for trusted_header in &proxies.trusted_headers {
                    match trusted_header {
                        ProxyIpAddressHeaders::Forwarded => {
                            p.add_trusted_header(HttpProxyHeaders::Forwarded)
                        }
                        ProxyIpAddressHeaders::XForwardedFor => {
                            p.add_trusted_header(HttpProxyHeaders::XForwardedFor)
                        }
                        ProxyIpAddressHeaders::XForwardedHost => {
                            p.add_trusted_header(HttpProxyHeaders::XForwardedHost)
                        }
                        ProxyIpAddressHeaders::XForwardedProto => {
                            p.add_trusted_header(HttpProxyHeaders::XForwardedProto)
                        }
                        ProxyIpAddressHeaders::XForwardedBy => {
                            p.add_trusted_header(HttpProxyHeaders::XForwardedBy)
                        }
                    };
                }
            })
            .build(),
    };

    Some((ref_, filter))
}
