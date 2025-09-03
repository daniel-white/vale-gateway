use crate::http::filters::collector::HttpFilters;
use crate::http::routes::controllers::HttpRouteInfo;
use crate::http::routes::filters::add_http_route_rule_filters;
use crate::http::routes::matches::add_http_route_rule_matches;
use gateway_api::httproutes::HTTPRouteRules;
use vg_core::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleKey};
use crate::http::routes::backends::add_http_route_rule_backends;

pub fn convert_http_route_rule(
    key: &HttpRouteRuleKey,
    route: &HttpRouteInfo,
    (rule, rule_idx): (&HTTPRouteRules, usize),
    http_filters: &HttpFilters,
    builder: &mut HttpRouteRuleBuilder,
) {
    add_http_route_rule_matches(rule, builder);
    add_http_route_rule_filters(key, http_filters, builder);
    add_http_route_rule_backends(key, route, builder);
}
