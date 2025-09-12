use gateway_api::common::HeaderModifier;
use http::{HeaderName, HeaderValue};
use tracing::warn;
use vg_core::http::filters::header_modifier::HttpHeaderModifierFilter;

pub fn convert_request_header_modifier(_modifier: &HeaderModifier) -> HttpHeaderModifierFilter {
    warn!("Request header modifier conversion is not yet implemented for the new gateway API");
    HttpHeaderModifierFilter::builder().build()
}

pub fn convert_response_header_modifier(_modifier: &HeaderModifier) -> HttpHeaderModifierFilter {
    warn!("Response header modifier conversion is not yet implemented for the new gateway API");
    HttpHeaderModifierFilter::builder().build()
}
