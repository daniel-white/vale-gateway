use crate::api::v1::http::filter::header_modifier::config::{
    HeaderModifierFilterConversionError, HeaderModifierWrapper,
};
use crate::api::v1::http::filter::http_route_url_rewrite::config::{
    BackendUriRewriterConversionError, HTTPRouteUrlRewriteWrapper,
};
use crate::api::v1::http::filter::request_redirect::config::{
    RedirectResponseFilterConversionError, RequestRedirectWrapper,
};
use crate::resources::{AccessControlFilterRef, ErrorResponseFilterRef, StaticResponseFilterRef};
use gateway_api::common::{GatewayInfrastructureParametersReference, HTTPFilterType};
use gateway_api::httproutes::HTTPRouteFilter;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use vg_http_config::filter::header_modifier::HeaderModifierFilter;
use vg_http_config::filter::redirect_response::RedirectResponseFilter;
use vg_http_config::routing::rule::filter::{
    AccessControlRuleFilter, BackendUriRewriterRuleFilter, ErrorResponseRuleFilter,
    RedirectResponseRuleFilter, RequestHeaderModifierRuleFilter, RuleFilter,
    StaticResponseRuleFilter,
};

type ExtensionRef = GatewayInfrastructureParametersReference;

#[derive(Debug, TypedBuilder)]
pub struct HTTPRouteFilterWrapper<'a> {
    namespace: &'a str,
    filter: &'a HTTPRouteFilter,
}

#[derive(Debug, Error)]
pub enum RuleFilterConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Unsupported filter type: {0:?}")]
    Unsupported(HTTPFilterType),
    #[error("Invalid extension: {0}")]
    Extension(#[from] ExtensionRuleFilterConversionError),
    #[error("Invalid request header modifier")]
    RequestHeaderModifier(HeaderModifierFilterConversionError),
    #[error("Invalid response header modifier")]
    ResponseHeaderModifier(HeaderModifierFilterConversionError),
    #[error("Invalid redirect configuration")]
    RequestRedirect(#[from] RedirectResponseFilterConversionError),
    #[error("Invalid URL rewrite configuration")]
    UrlRewrite(#[from] BackendUriRewriterConversionError),
}

impl TryFrom<HTTPRouteFilterWrapper<'_>> for RuleFilter {
    type Error = RuleFilterConversionError;

    fn try_from(value: HTTPRouteFilterWrapper<'_>) -> Result<Self, Self::Error> {
        let namespace = value.namespace;
        let filter = value.filter;

        match (
            &filter.r#type,
            &filter.extension_ref,
            &filter.request_header_modifier,
            &filter.response_header_modifier,
            &filter.request_redirect,
            &filter.url_rewrite,
        ) {
            (HTTPFilterType::ExtensionRef, Some(extension_ref), None, None, None, None) => {
                let extension_ref = ExtensionRefWrapper::builder()
                    .namespace(namespace)
                    .extension_ref(extension_ref)
                    .build();
                Self::try_from(extension_ref).map_err(RuleFilterConversionError::Extension)
            }
            (
                HTTPFilterType::RequestHeaderModifier,
                None,
                Some(request_header_modifier),
                None,
                None,
                None,
            ) => {
                let request_header_modifier: HeaderModifierWrapper = request_header_modifier.into();
                let filter = HeaderModifierFilter::try_from(request_header_modifier)
                    .map_err(RuleFilterConversionError::RequestHeaderModifier)?;
                let filter: RequestHeaderModifierRuleFilter = filter.into();
                Ok(filter.into())
            }
            (
                HTTPFilterType::ResponseHeaderModifier,
                None,
                None,
                Some(response_header_modifier),
                None,
                None,
            ) => {
                let response_header_modifier: HeaderModifierWrapper =
                    response_header_modifier.into();
                let filter = HeaderModifierFilter::try_from(response_header_modifier)
                    .map_err(RuleFilterConversionError::ResponseHeaderModifier)?;
                let filter: RequestHeaderModifierRuleFilter = filter.into();
                Ok(filter.into())
            }
            (HTTPFilterType::RequestRedirect, None, None, None, Some(request_redirect), None) => {
                let request_redirect: RequestRedirectWrapper = request_redirect.into();
                let filter = RedirectResponseFilter::try_from(request_redirect)?;
                let filter: RedirectResponseRuleFilter = filter.into();
                Ok(filter.into())
            }
            (HTTPFilterType::UrlRewrite, None, None, None, None, Some(url_rewrite)) => {
                let url_rewrite: HTTPRouteUrlRewriteWrapper = url_rewrite.into();
                let filter = BackendUriRewriterFilter::try_from(url_rewrite)?;
                let filter: BackendUriRewriterRuleFilter = filter.into();
                Ok(filter.into())
            }
            (HTTPFilterType::RequestMirror, None, None, None, None, None) => Err(
                RuleFilterConversionError::Unsupported(HTTPFilterType::RequestMirror),
            ),
            _ => Err(RuleFilterConversionError::InvalidConfiguration),
        }
    }
}

#[derive(Debug, TypedBuilder)]
pub struct ExtensionRefWrapper<'a> {
    namespace: &'a str,
    extension_ref: &'a ExtensionRef,
}

#[derive(Debug, Error)]
pub enum ExtensionRuleFilterConversionError {
    #[error("Unsupported extension kind: {0}.{1}")]
    Unsupported(String, String),
}

impl TryFrom<ExtensionRefWrapper<'_>> for RuleFilter {
    type Error = ExtensionRuleFilterConversionError;

    fn try_from(value: ExtensionRefWrapper) -> Result<Self, Self::Error> {
        let filter_ref = AccessControlFilterRef::try_from(&value);
        if let Ok(filter_ref) = filter_ref {
            let filter = AccessControlRuleFilter::builder().ref_(filter_ref).build();
            return Ok(filter.into());
        }

        let filter_ref = ErrorResponseFilterRef::try_from(&value);
        if let Ok(filter_ref) = filter_ref {
            let filter = ErrorResponseRuleFilter::builder().ref_(filter_ref).build();
            return Ok(filter.into());
        }

        let filter_ref = StaticResponseFilterRef::try_from(&value);
        if let Ok(filter_ref) = filter_ref {
            let filter = StaticResponseRuleFilter::builder().ref_(filter_ref).build();
            return Ok(filter.into());
        }

        Err(ExtensionRuleFilterConversionError::Unsupported(
            value.extension_ref.name.clone(),
            value.extension_ref.group.clone(),
        ))
    }
}

impl TryFrom<&ExtensionRefWrapper<'_>> for AccessControlFilterRef {
    type Error = ();

    fn try_from(value: &ExtensionRefWrapper) -> Result<Self, Self::Error> {
        if value.extension_ref.group == "vale-gateway.whitefamily.io"
            && value.extension_ref.name == "AccessControlFilter"
        {
            let ref_ = AccessControlFilterRef::new_named(
                value.namespace,
                value.extension_ref.name.as_str(),
            );
            Ok(ref_)
        } else {
            Err(())
        }
    }
}

impl TryFrom<&ExtensionRefWrapper<'_>> for ErrorResponseFilterRef {
    type Error = ();

    fn try_from(value: &ExtensionRefWrapper) -> Result<Self, Self::Error> {
        if value.extension_ref.group == "vale-gateway.whitefamily.io"
            && value.extension_ref.name == "ErrorResponseFilter"
        {
            let ref_ = ErrorResponseFilterRef::new_named(
                value.namespace,
                value.extension_ref.name.as_str(),
            );
            Ok(ref_)
        } else {
            Err(())
        }
    }
}

impl TryFrom<&ExtensionRefWrapper<'_>> for StaticResponseFilterRef {
    type Error = ();

    fn try_from(value: &ExtensionRefWrapper) -> Result<Self, Self::Error> {
        if value.extension_ref.group == "vale-gateway.whitefamily.io"
            && value.extension_ref.name == "StaticResponseFilter"
        {
            let ref_ = StaticResponseFilterRef::new_named(
                value.namespace,
                value.extension_ref.name.as_str(),
            );
            Ok(ref_)
        } else {
            Err(())
        }
    }
}
