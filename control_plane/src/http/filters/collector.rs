use crate::http::filters::extensions::HttpExtensionFilters;
use crate::http::filters::header_modifier::{
    convert_request_header_modifier, convert_response_header_modifier,
};
use crate::http::filters::response_redirect::convert_request_redirect;
use crate::http::filters::upstream_uri_rewrite::convert_url_rewrite;
use gateway_api::gateways::{Gateway, GatewayListeners};
use gateway_api::httproutes::{HTTPRoute, HTTPRouteRulesFilters, HTTPRouteRulesFiltersType};
use getset::Getters;
use std::collections::HashMap;
use vg_api::v1alpha1::{
    GatewayListener, GatewayListenerHttpFilterType, GatewayListenerHttpFilters,
};
use vg_core::http::filters::access_control::HttpAccessControlFilterKey;
use vg_core::http::filters::client_addr::HttpClientAddrFilterKey;
use vg_core::http::filters::error_response::HttpErrorResponseFilterKey;
use vg_core::http::filters::static_response::HttpStaticResponseFilterKey;
use vg_core::http::filters::HttpExtensionFilterKind;
use vg_core::http::listeners::{HttpFilterDefinition, HttpFilterDefinitionKey, HttpListenerFilter};
use vg_core::http::routes::rules::{HttpRouteRuleFilter, HttpRouteRuleKey};

#[derive(Debug, Getters)]
pub struct HttpFilters {
    #[getset(get = "pub")]
    listener_filters: Vec<HttpListenerFilter>,
    #[getset(get = "pub")]
    route_rule_filters: HashMap<HttpRouteRuleKey, Vec<HttpRouteRuleFilter>>,
    #[getset(get = "pub")]
    filter_definitions: HashMap<HttpFilterDefinitionKey, HttpFilterDefinition>,
}

impl HttpFilters {
    fn new() -> Self {
        Self {
            listener_filters: Vec::new(),
            route_rule_filters: HashMap::new(),
            filter_definitions: HashMap::new(),
        }
    }
}

pub async fn collect_http_filters(
    gateway: &Gateway,
    listener: &GatewayListeners,
    listener_configuration: &GatewayListener,
    routes: &Vec<HTTPRoute>,
    extension_filters: &HttpExtensionFilters,
) -> Option<HttpFilters> {
    let mut filters = HttpFilters::new();
    let Some(http_listener) = listener_configuration.http.as_ref() else {
        return None;
    };

    for filter in &http_listener.filters {
        if let Some((filter, definition)) =
            collect_listener_filter(filter, extension_filters).await
        {
            filters.listener_filters.push(filter);
            if let Some((key, definition)) = definition {
                filters.filter_definitions.entry(key).or_insert(definition);
            }
        }
    }

    for http_route in routes {
        for (rule_idx, rule) in http_route
            .spec
            .rules
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .enumerate()
        {
            let rule_key: HttpRouteRuleKey = format!(
                "{}-{}-rule{}",
                http_route.metadata.namespace.as_ref().unwrap(),
                http_route.metadata.name.as_ref().unwrap(),
                rule_idx
            )
            .into();
            let rule_filters = filters
                .route_rule_filters
                .entry(rule_key)
                .or_insert_with(Vec::new);

            for filter in rule.filters.as_ref().unwrap_or(&Vec::new()) {
                let (filter, definition) = match collect_http_route_rule_filter(filter, http_route, extension_filters).await {
                    Some(value) => value,
                    None => continue,
                };

                rule_filters.push(filter);
                if let Some((key, definition)) = definition {
                    filters.filter_definitions.entry(key).or_insert(definition);
                }
            }
        }
    }

    Some(filters)
}

async fn collect_http_route_rule_filter(filter: &HTTPRouteRulesFilters, http_route: &HTTPRoute, extension_filters: &HttpExtensionFilters) -> Option<(HttpRouteRuleFilter, Option<(HttpFilterDefinitionKey, HttpFilterDefinition)>)> {
    let (filter, definition) = match filter.r#type {
        HTTPRouteRulesFiltersType::RequestHeaderModifier => {
            if let Some(filter) = &filter.request_header_modifier {
                let filter = convert_request_header_modifier(filter);
                (
                    HttpRouteRuleFilter::UpstreamRequestHeaderModifier(filter),
                    None,
                )
            } else {
                return None;
            }
        }
        HTTPRouteRulesFiltersType::ResponseHeaderModifier => {
            if let Some(filter) = &filter.response_header_modifier {
                let filter = convert_response_header_modifier(filter);
                (HttpRouteRuleFilter::ResponseHeaderModifier(filter), None)
            } else {
                return None;
            }
        }
        HTTPRouteRulesFiltersType::RequestRedirect => {
            if let Some(filter) = &filter.request_redirect {
                let filter = convert_request_redirect(filter);
                (HttpRouteRuleFilter::RedirectResponse(filter), None)
            } else {
                return None;
            }
        }
        HTTPRouteRulesFiltersType::UrlRewrite => {
            if let Some(filter) = &filter.url_rewrite {
                let filter = convert_url_rewrite(filter);
                (HttpRouteRuleFilter::UpstreamUriRewrite(filter), None)
            } else {
                return None;
            }
        }
        HTTPRouteRulesFiltersType::ExtensionRef => {
            if let Some(extension_ref) = &filter.extension_ref
                && extension_ref.group == "vale-gateway.whitefamily.in"
            {
                match HttpExtensionFilterKind::try_from(extension_ref.kind.as_str()) {
                    Ok(HttpExtensionFilterKind::StaticResponse) => {
                        let key: HttpStaticResponseFilterKey = format!(
                            "{}-{}",
                            http_route.metadata.namespace.unwrap(),
                            &extension_ref.name
                        )
                            .into();
                        if let Some(filter) =
                            extension_filters.get_static_response_filter(&key).await
                        {
                            (
                                HttpRouteRuleFilter::StaticResponse(key.clone().into()),
                                Some((
                                    HttpFilterDefinitionKey::StaticResponse(key),
                                    HttpFilterDefinition::StaticResponse(filter),
                                )),
                            )
                        } else {
                            return None;
                        }
                    }
                    _ => {
                        return None;
                    }
                }
            } else {
                return None;
            }
        }
        _ => {
            return None;
            // Other filter types are not supported at the route level
        }
    };
    Some((filter, definition))
}

async fn collect_listener_filter(
    filter: &GatewayListenerHttpFilters,
    extension_filters: &HttpExtensionFilters,
) -> Option<(
    HttpListenerFilter,
    Option<(HttpFilterDefinitionKey, HttpFilterDefinition)>,
)> {
    let result = match filter.r#type {
        GatewayListenerHttpFilterType::RequestHeaderModifier => {
            if let Some(filter) = &filter.request_header_modifier {
                let filter = convert_request_header_modifier(filter);
                (
                    HttpListenerFilter::UpstreamRequestHeaderModifier(filter),
                    None,
                )
            } else {
                return None;
            }
        }
        GatewayListenerHttpFilterType::ResponseHeaderModifier => {
            if let Some(filter) = &filter.response_header_modifier {
                let filter = convert_response_header_modifier(filter);
                (HttpListenerFilter::ResponseHeaderModifier(filter), None)
            } else {
                return None;
            }
        }
        GatewayListenerHttpFilterType::RequestRedirect => {
            if let Some(filter) = &filter.request_redirect {
                let filter = convert_request_redirect(filter);
                (HttpListenerFilter::RedirectResponse(filter), None)
            } else {
                return None;
            }
        }
        GatewayListenerHttpFilterType::ErrorResponse => {
            if let Some(filter) = &filter.error_response {
                let key: HttpErrorResponseFilterKey =
                    format!("{}-{}", filter.namespace.as_ref().unwrap(), &filter.name).into();

                if let Some(filter) = extension_filters.get_error_response_filter(&key).await {
                    (
                        HttpListenerFilter::ErrorResponse(key.clone().into()),
                        Some((
                            HttpFilterDefinitionKey::ErrorResponse(key),
                            HttpFilterDefinition::ErrorResponse(filter),
                        )),
                    )
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        GatewayListenerHttpFilterType::ClientAddress => {
            if let Some(filter) = &filter.client_address {
                let key: HttpClientAddrFilterKey =
                    format!("{}-{}", filter.namespace.as_ref().unwrap(), &filter.name).into();

                if let Some(filter) = extension_filters.get_client_addr_filter(&key).await {
                    (
                        HttpListenerFilter::ClientAddr(key.clone().into()),
                        Some((
                            HttpFilterDefinitionKey::ClientAddr(key),
                            HttpFilterDefinition::ClientAddr(filter),
                        )),
                    )
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        GatewayListenerHttpFilterType::AccessControl => {
            if let Some(filter) = &filter.access_control {
                let key: HttpAccessControlFilterKey =
                    format!("{}-{}", filter.namespace.as_ref().unwrap(), &filter.name).into();

                if let Some(filter) = extension_filters.get_access_control_filter(&key).await {
                    (
                        HttpListenerFilter::AccessControl(key.clone().into()),
                        Some((
                            HttpFilterDefinitionKey::AccessControl(key.into()),
                            HttpFilterDefinition::AccessControl(filter),
                        )),
                    )
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };
    Some(result)
}
