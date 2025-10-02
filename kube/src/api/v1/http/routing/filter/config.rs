use derive_more::{Deref, From};
use gateway_api::common::{GatewayInfrastructureParametersReference, HTTPFilterType, HeaderModifier};
use gateway_api::httproutes::HTTPRouteFilter;
use thiserror::Error;
use vg_http_config::filter::{RedirectResponseRequestFilter, RequestFilter, RequestHeaderModifierRequestFilter, UpstreamUriRewriteRequestFilter};
use vg_http_config::filter::header_modifier::HeaderModifierFilter;
use vg_http_config::filter::redirect_response::RedirectResponseFilter;
use vg_http_config::filter::upstream_uri_rewrite::UpstreamUriRewriteFilter;
use crate::api::v1::http::filter::header_modifier::config::{HeaderModifierFilterConversionError, HeaderModifierWrapper};
use crate::api::v1::http::filter::http_route_url_rewrite::config::{HTTPRouteUrlRewriteWrapper, UpstreamUriRewriteConversionError};
use crate::api::v1::http::filter::request_redirect::config::{RedirectResponseFilterConversionError, RequestRedirectWrapper};

#[derive(Debug, Deref, From)]
pub struct HTTPRouteFilterWrapper<'a>(&'a HTTPRouteFilter);


#[derive(Debug, Error)]
pub enum RequestFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid request header modifier")]
    RequestHeaderModifier(HeaderModifierFilterConversionError),
    #[error("Invalid response header modifier")]
    ResponseHeaderModifier(HeaderModifierFilterConversionError),
    #[error("Invalid redirect configuration")]
    RequestRedirect(#[from]RedirectResponseFilterConversionError),
    #[error("Invalid URL rewrite configuration")]
    UrlRewrite(#[from]UpstreamUriRewriteConversionError),
}

impl TryFrom<HTTPRouteFilterWrapper<'_>> for Option<RequestFilter> {
    type Error = RequestFilterConversionError;

    fn try_from(value: HTTPRouteFilterWrapper) -> Result<Self, Self::Error> {
        match (&value.r#type, &value.extension_ref, &value.request_header_modifier, &value.response_header_modifier,&value.request_redirect, &value.url_rewrite) {
            (HTTPFilterType::ExtensionRef, Some(extension_ref), None, None, None, None) => {
                Err(RequestFilterConversionError::InvalidConfiguration)
            },
            (HTTPFilterType::RequestHeaderModifier, None, Some(request_header_modifier), None, None, None) => {
                let request_header_modifier: HeaderModifierWrapper = request_header_modifier.into();
                let filter = HeaderModifierFilter::try_from(request_header_modifier)
                    .map_err(RequestFilterConversionError::RequestHeaderModifier)?;
                let filter: RequestHeaderModifierRequestFilter = filter.into();
                Ok(Some(filter.into()))
            },
            (HTTPFilterType::ResponseHeaderModifier, None, None, Some(response_header_modifier), None, None) => {
                let response_header_modifier: HeaderModifierWrapper = response_header_modifier.into();
                let filter = HeaderModifierFilter::try_from(response_header_modifier)
                    .map_err(RequestFilterConversionError::ResponseHeaderModifier)?;
                let filter: RequestHeaderModifierRequestFilter = filter.into();
                Ok(Some(filter.into()))
            },
            (HTTPFilterType::RequestRedirect, None, None, None, Some(request_redirect), None) => {
                let request_redirect: RequestRedirectWrapper = request_redirect.into();
                let filter = RedirectResponseFilter::try_from(request_redirect)?;
                let filter: RedirectResponseRequestFilter = filter.into();
                Ok(Some(filter.into()))
            },
            (HTTPFilterType::UrlRewrite, None, None, None, None, Some(url_rewrite)) => {
                let url_rewrite: HTTPRouteUrlRewriteWrapper = url_rewrite.into();
                let filter = UpstreamUriRewriteFilter::try_from(url_rewrite)?;
                let filter: UpstreamUriRewriteRequestFilter = filter.into();
                Ok(Some(filter.into()))
            },
            _ => Err(RequestFilterConversionError::InvalidConfiguration),
        }
    }
}

#[derive(Debug, Deref, From)]
pub struct GatewayInfrastructureParametersReferenceWrapper<'a>(&'a GatewayInfrastructureParametersReference);