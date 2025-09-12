use gateway_api::common::HTTPRouteUrlRewrite;
use hickory_proto::rr::Name;
use std::str::FromStr;
use tracing::warn;
use vg_core::http::filters::upstream_uri_rewrite::HttpUpstreamUriRewriteFilter;

pub fn convert_url_rewrite(_rewrite: &HTTPRouteUrlRewrite) -> HttpUpstreamUriRewriteFilter {
    warn!("URL rewrite conversion is not yet implemented for the new gateway API");
    HttpUpstreamUriRewriteFilter::builder()
        .host(Some(Name::from_str("example.com").unwrap()))
        .build()
}
