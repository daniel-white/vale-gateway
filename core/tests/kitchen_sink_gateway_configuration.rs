use std::net::SocketAddr;
use std::sync::Arc;
use url::Url;
use vg_core::gateways::Gateway;
use vg_core::http::filters::access_control::{
    HttpAccessControlClients, HttpAccessControlFilter, HttpAccessControlFilterRef,
};
use vg_core::http::filters::client_addr::{HttpClientAddrFilter, HttpClientAddrFilterRef};
use vg_core::http::filters::error_response::{
    HttpErrorResponseFilter, HttpErrorResponseFilterRef, HttpProblemDetailErrorResponse,
};
use vg_core::http::filters::header_modifier::HttpHeaderModifierFilter;
use vg_core::http::filters::redirect_response::{
    HttpRedirectResponseFilter, HttpRedirectResponsePathRewrite,
};
use vg_core::http::filters::static_response::{HttpStaticResponseBody, HttpStaticResponseFilter};
use vg_core::http::listeners::{HttpBackend, HttpBackendEndpoint, HttpListener};
use vg_core::http::routes::backends::HttpRouteBackend;
use vg_core::ipc::IpcConfiguration;

#[test]
fn kitchen_sink_gateway_configuration() {
    use http::{HeaderName, HeaderValue, StatusCode};
    use std::net::IpAddr;
    use std::num::NonZeroU16;
    use std::str::FromStr;
    use vg_core::http::filters::access_control::HttpAccessControlEffect;
    use vg_core::http::filters::error_response::HttpErrorResponseKind;
    use vg_core::http::filters::redirect_response::HttpRedirectResponseKind;
    use vg_core::http::listeners::{HttpFilterDefinition, HttpListenerFilter};
    use vg_core::net::Port;

    // Build IPC configuration

    let ipc: SocketAddr = "127.0.0.1:9000".parse().expect("Invalid IPC address");
    let ipc = Arc::new(IpcConfiguration::builder().addr(ipc).build());

    // Representative values for filters
    let mut header_mod = HttpHeaderModifierFilter::builder();
    header_mod
        .add_header(
            HeaderName::from_static("x-add-header"),
            HeaderValue::from_static("add-value"),
        )
        .set_header(
            HeaderName::from_static("x-set-header"),
            HeaderValue::from_static("set-value"),
        )
        .remove_header(HeaderName::from_static("x-remove-header"));
    let header_mod = Arc::new(header_mod.build());

    let redirect = HttpRedirectResponseFilter::builder()
        .kind(HttpRedirectResponseKind::Temporary)
        .path(HttpRedirectResponsePathRewrite::PrefixMatch(
            "/v2".to_string(),
        ))
        .scheme(None)
        .host(None)
        .port(None)
        .build();
    let redirect = Arc::new(redirect);

    let access_clients = HttpAccessControlClients::builder()
        .ips(vec![IpAddr::from_str("127.0.0.1").unwrap()])
        .build();

    let access_control = HttpAccessControlFilter::builder()
        .key("access-key")
        .effect(HttpAccessControlEffect::Allow)
        .clients(access_clients)
        .build();
    let access_control = Arc::new(access_control);

    let mut client_addrs = HttpClientAddrFilter::builder();
    client_addrs
        .key("client-key")
        .trust_header(HeaderName::from_static("x-real-ip"));
    let client_addrs = client_addrs.build();
    let client_addrs = Arc::new(client_addrs);

    let error_response = HttpErrorResponseFilter::builder()
        .key("error-key")
        .kind(HttpErrorResponseKind::Html)
        .build();
    let error_response = Arc::new(error_response);

    let error_response_problem_detail = HttpErrorResponseFilter::builder()
        .key("error-key-problem-detail")
        .kind(HttpErrorResponseKind::ProblemDetail)
        .problem_detail(Some(
            HttpProblemDetailErrorResponse::builder()
                .authority(Url::from_str("https://example.com/").unwrap())
                .build(),
        ))
        .build();
    let error_response_problem_detail = Arc::new(error_response_problem_detail);

    let static_body = HttpStaticResponseBody::builder()
        .key("body-key")
        .content_type(HeaderValue::from_static("text/plain"))
        .build();
    let static_response = HttpStaticResponseFilter::builder()
        .key("static-key")
        .status_code(StatusCode::OK)
        .body(static_body)
        .build();
    let static_response = Arc::new(static_response);

    let access_control_ref = HttpAccessControlFilterRef::builder()
        .key(access_control.key().clone())
        .build();

    let client_addrs_ref = HttpClientAddrFilterRef::builder()
        .key(client_addrs.key().clone())
        .build();

    let error_response_ref = HttpErrorResponseFilterRef::builder()
        .key(error_response.key().clone())
        .build();

    let pd_error_response_ref = HttpErrorResponseFilterRef::builder()
        .key(error_response_problem_detail.key().clone())
        .build();
    
    let endpoint = HttpBackendEndpoint::builder()
        .node(Some("here".to_string()))
        .zone(Some("us-east-1a".to_string()))
        .addrs(         ["127.1.1.2", "192.168.27.4"]
                            .iter()
                            .filter_map(|a| IpAddr::from_str(a).ok())
                            .collect()).build();
        

    let backend = HttpBackend::builder()
        .kind("Service")
        .name("my-service")
        .namespace("default")
        .endpoints(vec![endpoint])
        .build();

    let mut http_listener = HttpListener::builder();
    http_listener
        .port(Port::new(NonZeroU16::new(8080).unwrap()))
        .add_filter(HttpListenerFilter::UpstreamRequestHeaderModifier(
            header_mod.clone(),
        ))
        .add_filter(HttpListenerFilter::ResponseHeaderModifier(
            header_mod.clone(),
        ))
        .add_filter(HttpListenerFilter::RedirectResponse(redirect.clone()))
        .add_filter(HttpListenerFilter::AccessControl(access_control_ref))
        .add_filter(HttpListenerFilter::ClientAddr(client_addrs_ref))
        .add_filter(HttpListenerFilter::ErrorResponse(error_response_ref))
        .add_filter(HttpListenerFilter::ErrorResponse(pd_error_response_ref))
        .add_filter_definition(HttpFilterDefinition::StaticResponse(
            static_response.clone(),
        ))
        .add_filter_definition(HttpFilterDefinition::AccessControl(access_control.clone()))
        .add_filter_definition(HttpFilterDefinition::ClientAddr(client_addrs.clone()))
        .add_filter_definition(HttpFilterDefinition::ErrorResponse(error_response.clone()))
        .add_filter_definition(HttpFilterDefinition::ErrorResponse(
            error_response_problem_detail.clone(),
        ))
        .add_route("my-route", |route| {
            route
                .add_host_header_match(|h| {
                    h.in_zone("example.com");
                })
                .add_rule("my-rule", |_, rule| {
                    rule.add_match(|m| {
                        m.add_exact_header(
                            HeaderName::from_static("content-type"),
                            HeaderValue::from_static("application/json"),
                        );
                    });

                    let backend = HttpRouteBackend::builder()
                        .weight(Some(80))
                        .port(Port::from_str("9090").ok())
                        .kind("Service")
                        .name("my-service")
                        .namespace("default")
                        .build();

                    rule.add_backend(backend);
                });
        })
        .add_backend(backend);
    let http_listener = Arc::new(http_listener.build());
    let gateway = Gateway::builder()
        .ipc(ipc)
        .http_listener(Some(http_listener))
        .build();

    let expected = serde_yaml::from_str(include_str!("cases/kitchen_sink_configuration.yaml"))
        .expect("Failed to deserialize expected YAML");

    assert_eq!(gateway, expected);
}
