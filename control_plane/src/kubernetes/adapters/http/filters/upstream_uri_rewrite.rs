use gateway_api::httproutes::HTTPRouteRulesFiltersUrlRewritePathType;
use gateway_api::httproutes::HTTPRouteRulesFiltersUrlRewrite;
use hickory_proto::rr::Name;
use vg_core::http::filters::upstream_uri_rewrite::{HttpUpstreamUriPathRewrite, HttpUpstreamUriRewriteFilter};

pub fn convert_url_rewrite(rewrite: &HTTPRouteRulesFiltersUrlRewrite) -> HttpUpstreamUriRewriteFilter {
    let hostname = if let Some(hostname) = &rewrite.hostname && !hostname.is_empty() {
        Name::from_utf8(hostname).ok()
    } else {
        None
    };
    
    let path = match rewrite.path.as_ref().cloned().map(|p| (p.r#type, p.replace_full_path, p.replace_prefix_match)) {
        Some((HTTPRouteRulesFiltersUrlRewritePathType::ReplaceFullPath, Some(full), _))   => 
        Some(HttpUpstreamUriPathRewrite::Full(full.clone())),
        Some((HTTPRouteRulesFiltersUrlRewritePathType::ReplacePrefixMatch, _, Some(prefix))) =>
        Some(HttpUpstreamUriPathRewrite::PrefixMatch(prefix.clone())),
        _ => None,
    };
    
    HttpUpstreamUriRewriteFilter::builder()
        .host(hostname)
        .path(path)
        .build()
}