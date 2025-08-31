use crate::kubernetes::KubeClientCell;
use crate::options::Options;
use crate::watch_objects;
use http::HeaderName;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use vg_api::v1alpha1::{
    ClientAddressFilter, ClientAddressFilterProxiesTrustedHeaders, ClientAddressFilterSource,
};
use vg_core::http::filters::client_addr::{
    HttpClientAddrFilter, HttpClientAddrFilterKey, HttpProxyHeaders,
};
use vg_core::sync::signal::{signal, Receiver};
use vg_core::task::Builder as TaskBuilder;
use vg_core::{await_ready, continue_on, ReadyState};

pub fn http_client_addr_filters(
    task_builder: &TaskBuilder,
    options: Arc<Options>,
    client_rx: &Receiver<KubeClientCell>,
) -> Receiver<HashMap<HttpClientAddrFilterKey, Arc<HttpClientAddrFilter>>> {
    let (tx, rx) = signal(stringify!(http_client_addr_filters));
    let client_address_filters_rx =
        watch_objects!(options, task_builder, ClientAddressFilter, client_rx);

    task_builder
        .new_task(stringify!(http_client_addr_filters))
        .spawn(async move {
            loop {
                if let ReadyState::Ready(filters) = await_ready!(client_address_filters_rx) {
                    let filters = filters
                        .iter()
                        .map(|(_, _, f)| convert(f.as_ref()))
                        .collect();

                    tx.set(filters).await;
                }
                continue_on!(client_address_filters_rx.changed());
            }
        });

    rx
}

fn convert(filter: &ClientAddressFilter) -> (HttpClientAddrFilterKey, Arc<HttpClientAddrFilter>) {
    let key = format!(
        "{}-{}",
        filter.metadata.name.as_deref().unwrap(),
        filter.metadata.namespace.as_deref().unwrap()
    );
    let filter = &filter.spec;

    let header = filter
        .header
        .as_ref()
        .and_then(|h| HeaderName::from_str(h.as_str()).ok());
    let proxies = filter.proxies.as_ref();

    let key: HttpClientAddrFilterKey = key.into();
    let mut builder = HttpClientAddrFilter::builder();
    builder.key(key.clone());

    let filter = match (&filter.source, header, proxies) {
        (ClientAddressFilterSource::Header, Some(header), _) => {
            builder.trust_header(header);
            builder.build()
        }
        (ClientAddressFilterSource::Proxies, _, Some(proxies)) => {
            builder.trust_proxies(|builder| {
                if proxies.trust_local_ranges {
                    builder.trust_local_ranges();
                }
                for trusted_ip in &proxies.trusted_ips {
                    builder.add_trusted_ip(*trusted_ip);
                }
                for trusted_range in &proxies.trusted_ranges {
                    builder.add_trusted_range(*trusted_range);
                }
                for trusted_header in &proxies.trusted_headers {
                    match trusted_header {
                        ClientAddressFilterProxiesTrustedHeaders::Forwarded => {
                            builder.add_trusted_header(HttpProxyHeaders::Forwarded)
                        }
                        ClientAddressFilterProxiesTrustedHeaders::XForwardedFor => {
                            builder.add_trusted_header(HttpProxyHeaders::XForwardedFor)
                        }
                        ClientAddressFilterProxiesTrustedHeaders::XForwardedHost => {
                            builder.add_trusted_header(HttpProxyHeaders::XForwardedHost)
                        }
                        ClientAddressFilterProxiesTrustedHeaders::XForwardedProto => {
                            builder.add_trusted_header(HttpProxyHeaders::XForwardedProto)
                        }
                        ClientAddressFilterProxiesTrustedHeaders::XForwardedBy => {
                            builder.add_trusted_header(HttpProxyHeaders::XForwardedBy)
                        }
                    };
                }
            });

            builder.build()
        }
        _ => panic!("Invalid ClientAddressFilter configuration"),
    };

    (key, Arc::new(filter))
}
