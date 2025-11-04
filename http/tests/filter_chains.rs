
use hickory_proto::rr::rdata::opt::EdnsCode::Chain;
use http::Request;
use std::error::Error;
use tower::Service;
use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
use vg_config::http::policy::client_addrs::TrustedProxyHeaderName::XForwardedBy;
use vg_config::http::policy::client_addrs::{ClientAddrExtractor, ClientAddrPolicy};
use vg_http::extensions::RequestSocketAddr;
use vg_http::filter::stage::inbound_request::EarlyInboundRequestFilterChainFactory;

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
        .effect(AccessControlEffect::Allow)
        .clients(vec![])
        .build();

    let mut chain = EarlyInboundRequestFilterChainFactory::with_client_addr(&client_addr)?
        .with_access_control(&access_control)?
        .chain();

    let r = chain.call(req).await;
    println!("{:?}", r);
    Ok(())
}
