use gateway_api::httproutes::{
    HTTPRouteRulesFiltersRequestRedirect, HTTPRouteRulesFiltersRequestRedirectPath,
    HTTPRouteRulesFiltersRequestRedirectPathType, HTTPRouteRulesFiltersRequestRedirectScheme,
};
use hickory_proto::rr::Name;
use http::uri::Scheme;
use std::num::NonZeroU16;
use std::sync::Arc;
use vg_core::http::filters::redirect_response::{
    HttpRedirectResponseFilter, HttpRedirectResponseKind, HttpRedirectResponsePathRewrite,
};
use vg_core::net::Port;

pub fn convert_request_redirect(
    redirect: &HTTPRouteRulesFiltersRequestRedirect,
) -> Arc<HttpRedirectResponseFilter> {
    let kind = match redirect.status_code {
        Some(code) if code == 301 => HttpRedirectResponseKind::Permanent,
        Some(code) if code == 302 => HttpRedirectResponseKind::Temporary,
        _ => HttpRedirectResponseKind::default(),
    };

    let scheme = redirect.scheme.as_ref().map(|s| match s {
        HTTPRouteRulesFiltersRequestRedirectScheme::Http => Scheme::HTTP,
        HTTPRouteRulesFiltersRequestRedirectScheme::Https => Scheme::HTTPS,
    });

    let host: Option<Name> = redirect
        .hostname
        .as_ref()
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .and_then(|h| h.parse().ok());

    let port = redirect
        .port
        .and_then(|p| NonZeroU16::new(p as u16))
        .map(Port::new);

    let path = match &redirect.path {
        Some(p) => convert_path_rewrite(p),
        None => None,
    };

    let filter = HttpRedirectResponseFilter::builder()
        .kind(kind)
        .scheme(scheme)
        .host(host)
        .port(port)
        .path(path)
        .build();
    
    Arc::new(filter)
}

/// Convert Gateway API path upstream_uri_rewrite configuration
fn convert_path_rewrite(
    path: &HTTPRouteRulesFiltersRequestRedirectPath,
) -> Option<HttpRedirectResponsePathRewrite> {
    match (
        &path.r#type,
        &path.replace_full_path,
        &path.replace_prefix_match,
    ) {
        (HTTPRouteRulesFiltersRequestRedirectPathType::ReplaceFullPath, Some(path), _) => {
            Some(HttpRedirectResponsePathRewrite::Full(path.clone()))
        }
        (HTTPRouteRulesFiltersRequestRedirectPathType::ReplacePrefixMatch, _, Some(path)) => {
            Some(HttpRedirectResponsePathRewrite::PrefixMatch(path.clone()))
        }
        _ => None,
    }
}
