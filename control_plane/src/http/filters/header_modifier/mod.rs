use gateway_api::httproutes::{HTTPRouteRulesFiltersRequestHeaderModifier, HTTPRouteRulesFiltersResponseHeaderModifier};
use http::{HeaderName, HeaderValue};
use vg_core::http::filters::header_modifier::HttpHeaderModifierFilter;
use std::str::FromStr;

pub fn convert_request_header_modifier(
    modifier: &HTTPRouteRulesFiltersRequestHeaderModifier,
) -> HttpHeaderModifierFilter {
    let mut builder = HttpHeaderModifierFilter::builder();

    for add in modifier.add.clone().unwrap_or_default() {
        let name = HeaderName::from_str(add.name.as_str()).unwrap();
        let value = HeaderValue::from_str(add.value.as_str()).unwrap();
        builder.add_header(name, value);
    }

    for set in &modifier.set.clone().unwrap_or_default() {
        let name = HeaderName::from_str(set.name.as_str()).unwrap();
        let value = HeaderValue::from_str(set.value.as_str()).unwrap();
        builder.set_header(name, value);
    }

    for name in &modifier.remove.clone().unwrap_or_default() {
        let name = HeaderName::from_str(name.as_str()).unwrap();
        builder.remove_header(name);
    }

    builder.build()
}

pub fn convert_response_header_modifier(
    modifier: &HTTPRouteRulesFiltersResponseHeaderModifier,
) -> HttpHeaderModifierFilter {
    let mut builder = HttpHeaderModifierFilter::builder();

    for add in modifier.add.clone().unwrap_or_default() {
        let name = HeaderName::from_str(add.name.as_str()).unwrap();
        let value = HeaderValue::from_str(add.value.as_str()).unwrap();
        builder.add_header(name, value);
    }

    for set in &modifier.set.clone().unwrap_or_default() {
        let name = HeaderName::from_str(set.name.as_str()).unwrap();
        let value = HeaderValue::from_str(set.value.as_str()).unwrap();
        builder.set_header(name, value);
    }

    for name in &modifier.remove.clone().unwrap_or_default() {
        let name = HeaderName::from_str(name.as_str()).unwrap();
        builder.remove_header(name);
    }

    builder.build()
}
