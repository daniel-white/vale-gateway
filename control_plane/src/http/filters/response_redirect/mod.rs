use gateway_api::common::{RequestRedirect, RequestRedirectPath};
use hickory_proto::rr::Name;
use http::uri::Scheme;
use std::num::NonZeroU16;
use std::str::FromStr;
use tracing::warn;
use vg_core::http::filters::redirect_response::{
    HttpRedirectResponseFilter, HttpRedirectResponseKind, HttpRedirectResponsePathRewrite,
};

pub fn convert_request_redirect(_redirect: &RequestRedirect) -> HttpRedirectResponseFilter {
    warn!("Request redirect conversion is not yet implemented for the new gateway API");
    HttpRedirectResponseFilter::builder()
        .kind(HttpRedirectResponseKind::Temporary)
        .scheme(http::uri::Scheme::HTTP)
        .host(Some(Name::from_str("example.com").unwrap()))
        .port(Some(NonZeroU16::new(80).unwrap().into()))
        .path(Some(HttpRedirectResponsePathRewrite::Full("/".to_string())))
        .build()
}

fn convert_scheme(_scheme: &str) -> Scheme {
    Scheme::HTTP
}

fn convert_path(_path: &RequestRedirectPath) -> String {
    "/".to_string()
}
