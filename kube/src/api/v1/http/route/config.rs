use crate::api::v1::http::route::filter::config::{
    HTTPRouteFilterWrapper, RuleFilterConversionError,
};
use crate::api::v1::http::route::r#match::config::{
    RequestMatcherConversionError, RouteMatchWrapper,
};
use derive_more::{Deref, From};
use gateway_api::httproutes::{HTTPRouteRule, HTTPRouteTimeout};
use kube_core::Duration;
use kube_core::duration::ParseError;
use std::str::FromStr;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::policy::TimeoutPolicy;
use vg_http_config::routing::rule::Rule;
use vg_http_config::routing::rule::filter::RuleFilter;
use vg_http_config::routing::rule::matcher::RequestMatcher;
use vg_http_config::routing::rule::policy::{RulePolicies, TimeoutPolicies};

#[derive(Debug, TypedBuilder)]
pub struct HTTPRouteRuleWrapper<'a> {
    namespace: &'a str,
    rule: &'a HTTPRouteRule,
}

#[derive(Debug, Error)]
pub enum RuleConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Timeout policies conversion error: {0}")]
    TimeoutPolicies(#[from] TimeoutPoliciesConversionError),
    #[error("Matcher conversion error at index {0}: {1}")]
    Matcher(usize, RequestMatcherConversionError),
    #[error("Filter conversion error: {0}")]
    Filter(usize, RuleFilterConversionError),
}

impl TryFrom<HTTPRouteRuleWrapper<'_>> for Rule {
    type Error = RuleConversionError;

    fn try_from(value: HTTPRouteRuleWrapper) -> Result<Self, Self::Error> {
        let namespace = value.namespace;
        let rule = value.rule;

        let matchers = rule
            .matches
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, match_)| {
                let match_ = RouteMatchWrapper::from(match_);
                RequestMatcher::try_from(match_)
                    .map_err(|err| RuleConversionError::Matcher(idx, err))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let filters = rule
            .filters
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, filter)| {
                let filter = HTTPRouteFilterWrapper::builder()
                    .namespace(namespace)
                    .filter(filter)
                    .build();
                RuleFilter::try_from(filter).map_err(|err| RuleConversionError::Filter(idx, err))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let timeouts = rule.timeouts.as_ref().map(HTTPRouteTimeoutWrapper);
        let timeouts = timeouts
            .map(TimeoutPolicies::try_from)
            .transpose()?
            .unwrap_or_default();
        let policies = RulePolicies::builder()
            .timeouts(timeouts)
            .retries(None) // TODO: Implement retries conversion
            .build();

        let rule = Self::builder()
            .name(None)
            .matchers(matchers)
            .filters(filters) // TODO: Implement name conversion if needed
            .policies(policies)
            .build();

        Ok(rule)
    }
}

#[derive(Debug, Deref, From)]
pub struct HTTPRouteTimeoutWrapper<'a>(&'a HTTPRouteTimeout);

#[derive(Debug, Error)]
pub enum TimeoutPoliciesConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid request timeout: {0}")]
    RequestTimeout(ParseError),
    #[error("Invalid backend request timeout: {0}")]
    BackendRequestTimeout(ParseError),
}

impl TryFrom<HTTPRouteTimeoutWrapper<'_>> for TimeoutPolicies {
    type Error = TimeoutPoliciesConversionError;

    fn try_from(value: HTTPRouteTimeoutWrapper) -> Result<Self, Self::Error> {
        fn convert(s: Option<&str>) -> Result<Option<TimeoutPolicy>, ParseError> {
            let duration = s.map(Duration::from_str).transpose()?;
            Ok(duration.map(std::time::Duration::from).and_then(|to| {
                if to.is_zero() {
                    None
                } else {
                    let policy = TimeoutPolicy::builder().duration(to).build();
                    Some(policy)
                }
            }))
        }

        let request = convert(value.request.as_deref())
            .map_err(TimeoutPoliciesConversionError::RequestTimeout)?;
        let backend_request = convert(value.backend_request.as_deref())
            .map_err(TimeoutPoliciesConversionError::BackendRequestTimeout)?;

        let policies = Self::builder()
            .request(request)
            .backend_request(backend_request)
            .build();

        Ok(policies)
    }
}
