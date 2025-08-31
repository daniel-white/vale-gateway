use std::num::NonZeroU16;
use rules::convert_http_route_rule;
use gateway_api::gateways::Gateway;
use gateway_api::httproutes::HTTPRoute;
use vg_api::v1alpha1::GatewayListenerHttpFilters;
use vg_core::http::listeners::{HttpFilterDefinition, HttpListener, HttpListenerFilter};
use vg_core::http::routes::HttpRoute;

pub mod rules;
pub mod gateway;

pub fn convert_http_listener(
    gateway: &Gateway,
    filters: Vec<GatewayListenerHttpFilters>,
    routes: Vec<HTTPRoute>
) -> HttpListener {
    let spec = gateway.spec;

    let http_listener = spec
        .listeners
        .iter()
        .find(|listener| listener.protocol == "HTTP");

    let port: NonZeroU16 = if let Some(listener) = http_listener && listener.port > 0 {
        NonZeroU16::new(listener.port as u16).unwrap()
    } else {
        NonZeroU16::new(80).unwrap()
    };

    let mut builder = HttpListener::builder();

    builder.port(port);

    for filter in filters {
        match filter.t
        builder.add_filter(filter);
    }


    for route in routes {
        builder.route_builders.push(route.into());
    }


    builder.build()
}

pub fn convert_http_route(route: &HTTPRoute) -> HttpRoute {
    let key = format!(
        "{}-{}",
        route.metadata.namespace.as_deref().unwrap()
        route.metadata.name.as_deref().unwrap()
    );
    let route_spec = &route.spec;

    let mut builder = HttpRoute::builder(key);

    for hostname in &route_spec.hostnames.unwrap_or_default() {
        builder.add_host_header_match(|builder| {
            if hostname.starts_with("*.") {
                builder.in_zone(hostname.trim_start_matches("*."));
            } else {
                builder.fully_qualified(hostname);
            }
        });
    }

    for (rule_idx, rule) in route_spec.rules.unwrap_or_default().iter().enumerate() {
        let key = format!("{key}-rule-{rule_idx}");
        builder.add_rule(key, |builder| {
            convert_http_route_rule(route, (rule, rule_idx), builder);
        });
    }

    builder.build()
}
