use crate::http::filters::header_modifier::{
    convert_request_header_modifier, convert_response_header_modifier,
};
use crate::http::filters::response_redirect::convert_request_redirect;
use crate::http::filters::static_response::convert_static_response_ref;
use crate::http::filters::upstream_uri_rewrite::convert_url_rewrite;
use gateway_api::httproutes::{
    HTTPRoute, HTTPRouteRules, HTTPRouteRulesFiltersType, HTTPRouteRulesMatches,
    HTTPRouteRulesMatchesHeadersType, HTTPRouteRulesMatchesMethod, HTTPRouteRulesMatchesPathType,
    HTTPRouteRulesMatchesQueryParamsType,
};
use http::{HeaderName, HeaderValue};
use std::str::FromStr;
use tracing::warn;
use vg_core::http::filters::ExtensionFilterKind;
use vg_core::http::matches::{HttpMethodMatch, HttpRouteRuleMatchesBuilder};
use vg_core::http::routes::rules::{HttpRouteRuleBuilder, HttpRouteRuleFilter};

pub fn convert_http_route_rule(
    route: &HTTPRoute,
    (rule, rule_idx): (&HTTPRouteRules, usize),
    builder: &mut HttpRouteRuleBuilder,
) {
    add_http_route_rules_filters(route, (rule, rule_idx), builder);
    add_http_route_rules_matches(rule, builder);

    // if let Some(backends) = &rule.backend_refs {
    //     for backend in backends {
    //         if let Some(service) = &backend.service {
    //             let name = service.name.as_str();
    //             let port = backend.port.unwrap_or(80) as u16;
    //             let weight = backend.weight.unwrap_or(1);
    //
    //             builder.add_backend(name, port, weight);
    //         } else {
    //             warn!(
    //                 "Unsupported backend_ref {:?} for HTTPRoute {:?} at rule index {}",
    //                 backend, route.metadata.name, rule_idx
    //             );
    //         }
    //     }
    // } else {
    //     warn!(
    //         "No backend_refs specified for HTTPRoute {:?} at rule index {}",
    //         route.metadata.name, rule_idx
    //     );
    //     return None;
    // }
}

fn add_http_route_rules_filters(
    route: &HTTPRoute,
    (rule, rule_idx): (&HTTPRouteRules, usize),
    builder: &mut HttpRouteRuleBuilder,
) {
    for filter in rule.filters.as_ref().unwrap_or(&Vec::new()) {
        match filter.r#type {
            HTTPRouteRulesFiltersType::RequestHeaderModifier => {
                if let Some(request_header_modifier) = &filter.request_header_modifier {
                    let filter = convert_request_header_modifier(request_header_modifier);
                    builder.add_filter(HttpRouteRuleFilter::UpstreamRequestHeaderModifier(filter));
                }
            }
            HTTPRouteRulesFiltersType::ResponseHeaderModifier => {
                if let Some(response_header_modifier) = &filter.response_header_modifier {
                    let filter = convert_response_header_modifier(response_header_modifier);
                    builder.add_filter(HttpRouteRuleFilter::ResponseHeaderModifier(filter));
                }
            }
            HTTPRouteRulesFiltersType::RequestRedirect => {
                if let Some(request_redirect) = &filter.request_redirect {
                    let filter = convert_request_redirect(request_redirect);
                    builder.add_filter(HttpRouteRuleFilter::RedirectResponse(filter));
                }
            }
            HTTPRouteRulesFiltersType::UrlRewrite => {
                if let Some(url_rewrite) = &filter.url_rewrite {
                    let filter = convert_url_rewrite(url_rewrite);
                    builder.add_filter(HttpRouteRuleFilter::UpstreamUriRewrite(filter));
                }
            }
            HTTPRouteRulesFiltersType::ExtensionRef => {
                if let Some(extension_ref) = &filter.extension_ref
                    && extension_ref.group == "vale-gateway.whitefamily.in"
                {
                    match ExtensionFilterKind::try_from(extension_ref.kind.as_str()) {
                        Ok(ExtensionFilterKind::StaticResponse) => {
                            let filter = convert_static_response_ref(extension_ref);
                            builder.add_filter(HttpRouteRuleFilter::StaticResponse(filter));
                        }
                        _ => {
                            warn!(
                                "Unsupported extension filter kind {:?} for HTTPRoute {:?} at rule index {}",
                                extension_ref.kind, route.metadata.name, rule_idx
                            );
                        }
                    }
                } else {
                    warn!(
                        "Unsupported extension filter {:?} for HTTPRoute {:?} at rule index {}",
                        filter.extension_ref, route.metadata.name, rule_idx
                    );
                }
            }
            HTTPRouteRulesFiltersType::RequestMirror => {
                warn!(
                    "RequestMirror filter type is not supported yet for HTTPRoute {:?} at rule index {}",
                    route.metadata.name, rule_idx
                );
            }
            _ => {
                warn!(
                    "Unsupported filter type {:?} for HTTPRoute {:?} at rule index {}",
                    filter.r#type, route.metadata.name, rule_idx
                );
            }
        }
    }
}

fn add_http_route_rules_matches(rule: &HTTPRouteRules, builder: &mut HttpRouteRuleBuilder) {
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
