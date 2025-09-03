use gateway_api::httproutes::{
    HTTPRouteRules, HTTPRouteRulesMatches, HTTPRouteRulesMatchesHeadersType,
    HTTPRouteRulesMatchesMethod, HTTPRouteRulesMatchesPathType,
    HTTPRouteRulesMatchesQueryParamsType,
};
use http::{HeaderName, HeaderValue};
use std::str::FromStr;
use tracing::warn;
use vg_core::http::matches::{HttpMethodMatch, HttpRouteRuleMatchesBuilder};
use vg_core::http::routes::rules::HttpRouteRuleBuilder;

pub fn add_http_route_rule_matches(rule: &HTTPRouteRules, builder: &mut HttpRouteRuleBuilder) {
    if let Some(matches) = &rule.matches {
        for matches in matches {
            builder.add_match(|builder| {
                add_method_match(matches, builder);
                add_path_match(matches, builder);
                add_header_matches(matches, builder);
                add_query_params_matches(matches, builder);
            });
        }
    }
}

fn add_method_match(matches: &HTTPRouteRulesMatches, builder: &mut HttpRouteRuleMatchesBuilder) {
    if let Some(method) = &matches.method {
        let method = match method {
            HTTPRouteRulesMatchesMethod::Get => HttpMethodMatch::Get,
            HTTPRouteRulesMatchesMethod::Head => HttpMethodMatch::Head,
            HTTPRouteRulesMatchesMethod::Post => HttpMethodMatch::Post,
            HTTPRouteRulesMatchesMethod::Put => HttpMethodMatch::Put,
            HTTPRouteRulesMatchesMethod::Delete => HttpMethodMatch::Delete,
            HTTPRouteRulesMatchesMethod::Connect => HttpMethodMatch::Connect,
            HTTPRouteRulesMatchesMethod::Options => HttpMethodMatch::Options,
            HTTPRouteRulesMatchesMethod::Trace => HttpMethodMatch::Trace,
            HTTPRouteRulesMatchesMethod::Patch => HttpMethodMatch::Patch,
        };

        builder.with_method(method);
    }
}

fn add_path_match(matches: &HTTPRouteRulesMatches, builder: &mut HttpRouteRuleMatchesBuilder) {
    if let Some(path) = &matches.path {
        match (path.r#type.as_ref(), path.value.as_ref()) {
            (Some(HTTPRouteRulesMatchesPathType::Exact), Some(value)) => {
                builder.with_exact_path(value);
            }
            (Some(HTTPRouteRulesMatchesPathType::PathPrefix), Some(value)) => {
                builder.with_path_prefix(value);
            }
            (Some(HTTPRouteRulesMatchesPathType::RegularExpression), Some(value)) => {
                builder.with_path_matching(value);
            }
            _ => {
                warn!("Unsupported path match type or missing value: {:?}", path);
            }
        }
    } else {
        warn!("No path match specified in source: {:?}", matches);
    }
}

fn add_header_matches(matches: &HTTPRouteRulesMatches, builder: &mut HttpRouteRuleMatchesBuilder) {
    for header in matches.headers.iter().flatten() {
        match header
            .r#type
            .as_ref()
            .unwrap_or(&HTTPRouteRulesMatchesHeadersType::Exact)
        {
            HTTPRouteRulesMatchesHeadersType::Exact => {
                let name = HeaderName::from_str(header.name.as_str()).unwrap();
                let value = HeaderValue::from_str(header.value.as_str()).unwrap();
                builder.add_exact_header(name, value);
            }
            HTTPRouteRulesMatchesHeadersType::RegularExpression => {
                let name = HeaderName::from_str(header.name.as_str()).unwrap();
                builder.add_header_matching(name, &header.value);
            }
        }
    }
}

fn add_query_params_matches(
    matches: &HTTPRouteRulesMatches,
    builder: &mut HttpRouteRuleMatchesBuilder,
) {
    for query_param in matches.query_params.iter().flatten() {
        match query_param
            .r#type
            .as_ref()
            .unwrap_or(&HTTPRouteRulesMatchesQueryParamsType::Exact)
        {
            HTTPRouteRulesMatchesQueryParamsType::Exact => {
                builder
                    .add_exact_query_param(query_param.name.as_str(), query_param.value.as_str());
            }
            HTTPRouteRulesMatchesQueryParamsType::RegularExpression => {
                builder.add_query_param_matching(
                    query_param.name.as_str(),
                    query_param.value.as_str(),
                );
            }
        }
    }
}
