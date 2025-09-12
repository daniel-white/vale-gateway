use crate::http::filters::collector::HttpRouteFilters;
use crate::http::routes::backends::add_http_route_rule_backends;
use crate::http::routes::controllers::HttpRouteInfo;
use crate::http::routes::filters::add_http_route_rule_filters;
use crate::http::routes::matches::add_http_route_rule_matches;
use gateway_api::apis::standard::httproutes::HTTPRouteRule;
use vg_core::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleKey};

pub fn convert_http_route_rule(
    key: &HttpRouteRuleKey,
    route: &HttpRouteInfo,
    (rule, rule_idx): (&HTTPRouteRule, usize),
    http_filters: &HttpRouteFilters,
    builder: &mut HttpRouteRuleBuilder,
) {
    add_http_route_rule_matches(rule, builder);
    add_http_route_rule_filters(key, http_filters, builder);
    add_http_route_rule_backends(key, route, builder);
}
