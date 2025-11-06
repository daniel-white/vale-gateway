use hickory_proto::rr::rdata::opt::EdnsCode::Chain;
use http::{HeaderMap, HeaderValue, Request};
use std::error::Error;
use tower::Service;
use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
use vg_config::http::filter::header_modifier::{HeaderModifierFilter, RequestHeaderModifierFilter};
use vg_config::http::listener::filter::RequestHeaderModifierListenerFilter;
use vg_config::http::policy::client_addrs::TrustedProxyHeaderName::XForwardedBy;
use vg_config::http::policy::client_addrs::{ClientAddrExtractor, ClientAddrPolicy};
use vg_config::http::route::rule::filter::RuleBackendFilter::RequestHeaderModifier;
use vg_http::extensions::RequestSocketAddr;
use vg_http::filter::SharedFilterHandlerLayer::HeaderModifier;
use vg_http::filter::stage::inbound_request::PreRoutingRequestFilterChainFactory;
use vg_http::filter::stage::pre_routing_request::PreRoutingRequestFilterChain;

#[tokio::test]
pub async fn early_factory() -> Result<(), Box<dyn Error>> {
    let req_ip = RequestSocketAddr::builder()
        .addr("127.0.0.1:1234".parse().unwrap())
        .build();

    let req = Request::builder().extension(req_ip).body(()).unwrap();

    let (req, _) = req.into_parts();

    let client_addr = ClientAddrPolicy::builder()
        .extractor(ClientAddrExtractor::Direct)
        .backend_header(None)
        .build();

    let access_control = AccessControlFilter::builder()
        .effect(AccessControlEffect::Deny)
        .clients(vec![])
        .build();
    
    let mut hm = HeaderMap::new();
    hm.insert("foo", HeaderValue::from_static("bar"));
    let hm = HeaderModifierFilter::builder()
        .add(hm)
        .remove(Vec::default())
        .set(HeaderMap::new())
        .build();
    
    let hm: RequestHeaderModifierFilter = hm.into();
    let hm: RequestHeaderModifierListenerFilter = hm.into();

    let mut chain: PreRoutingRequestFilterChain = PreRoutingRequestFilterChainFactory::with_client_addr(&client_addr)?
        .add_access_control(&access_control)?
        .add_header_modifier(&hm)?
        .into();

    let r = chain.call(req).await;
    println!("{:?}", r);
    Ok(())
}
