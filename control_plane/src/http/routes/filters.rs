use crate::http::filters::collector::HttpFilters;
use vg_core::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleKey};

pub fn add_http_route_rule_filters(
    key: &HttpRouteRuleKey,
    http_filters: &HttpFilters,
    builder: &mut HttpRouteRuleBuilder,
) {
    if let Some(filters) = http_filters.route_rule_filters().get(key) {
        for filter in filters {
            builder.add_filter(filter.clone());
        }
    }
}
