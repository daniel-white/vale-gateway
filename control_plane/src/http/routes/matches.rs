use gateway_api::apis::standard::httproutes::HTTPRouteRule;
use tracing::warn;
use vg_core::http::routes::rules::HttpRouteRuleBuilder;

pub fn add_http_route_rule_matches(_rule: &HTTPRouteRule, _builder: &mut HttpRouteRuleBuilder) {
    // TODO: Update this function to work with the new gateway API structure
    warn!("HTTP route rule matches are not yet implemented for the new gateway API");
}
