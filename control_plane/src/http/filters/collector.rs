use std::collections::HashMap;
use gateway_api::gateways::{Gateway, GatewayListeners};
use gateway_api::httproutes::HTTPRoute;
use getset::Getters;
use vg_api::v1alpha1::{GatewayListener, GatewayListenerHttpFilterType};
use vg_core::http::listeners::{HttpFilterDefinition, HttpListenerFilter};
use vg_core::http::routes::rules::{HttpRouteRuleFilter, HttpRouteRuleKey};
use vg_core::task::Builder as TaskBuilder;
use crate::http::filters::extensions::HttpExtensionFilters;
use crate::http::filters::header_modifier::{convert_request_header_modifier, convert_response_header_modifier};
use crate::http::filters::response_redirect::convert_request_redirect;

#[derive(Debug, Getters)]
pub struct HttpFilters {
    #[getset(get = "pub")]
    listener_filters: Vec<HttpListenerFilter>,
    #[getset(get = "pub")]
    route_rule_filters: HashMap<HttpRouteRuleKey, Vec<HttpRouteRuleFilter>>,
    #[getset(get = "pub")]
    filter_definitions: Vec<HttpFilterDefinition>
}

impl HttpFilters {
    fn new() -> Self {
        Self {
            listener_filters: Vec::new(),
            route_rule_filters: HashMap::new(),
            filter_definitions: Vec::new()
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
        let (filter, definition) = match filter.r#type {
            GatewayListenerHttpFilterType::RequestHeaderModifier => {
                if let Some(filter) = &filter.request_header_modifier {
                    let filter = convert_request_header_modifier(filter);
                    (HttpListenerFilter::UpstreamRequestHeaderModifier(filter), None)
                } else {
                    continue;
                }
            }
            GatewayListenerHttpFilterType::ResponseHeaderModifier => {
                if let Some(filter) = &filter.response_header_modifier {
                    let filter = convert_response_header_modifier(filter);
                    (HttpListenerFilter::ResponseHeaderModifier(filter), None)
                } else {
                    continue;
                }
            }
            GatewayListenerHttpFilterType::RequestRedirect => {
                if let Some(filter) = &filter.request_redirect {
                    let filter = convert_request_redirect(filter);
                    (HttpListenerFilter::RedirectResponse(filter), None)
                } else {
                    continue;
                }
            }
            GatewayListenerHttpFilterType::ErrorResponse => {
                if let Some(filter) = &filter.error_response {
                    if let Some(definitions) = extension_filters.get_error_response_filter().await.ok() {
                        if let Some(definition) = definitions.get(&filter.key()) {
                            let filter = HttpListenerFilter::ErrorResponse(definition.clone());
                            (filter, Some(HttpFilterDefinition::ErrorResponse(definition.clone())))
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }}
            _ => continue // TODO: handle other filter types
        };
        
        filters.listener_filters.push(filter);
        if let Some(definition) = definition {
            filters.filter_definitions.push(definition);
        }
    }
    
    
    Some(filters)
}