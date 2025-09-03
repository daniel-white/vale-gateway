use crate::http::filters::collector::HttpFilters;
use crate::http::routes::rules::convert_http_route_rule;
use vg_core::http::routes::HttpRouteBuilder;
use crate::http::routes::controllers::HttpRouteInfo;

pub fn convert_http_route(
    builder: &mut HttpRouteBuilder,
    key: &str,
    route: &HttpRouteInfo,
    http_filters: &HttpFilters,
) {
    let http_route = route.http_route();
    let http_route_spec = &http_route.spec;

    for hostname in http_route_spec.hostnames.as_ref().unwrap_or(&Vec::new()) {
        builder.add_host_header_match(|builder| {
            if hostname.starts_with("*.") {
                builder.in_zone(hostname.trim_start_matches("*."));
            } else {
                builder.fully_qualified(hostname);
            }
        });
    }

    for (rule_idx, rule) in http_route_spec
        .rules
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .enumerate()
    {
        let key = format!("{key}-rule-{rule_idx}");
        builder.add_rule(key, |key, builder| {
            convert_http_route_rule(key, route, (rule, rule_idx), http_filters, builder);
        });
    }
}
