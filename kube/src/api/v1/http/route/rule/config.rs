use crate::api::v1::http::route::filter::config::{
    HTTPRouteFilterWrapper, RuleFilterConversionError,
};
use crate::api::v1::http::route::r#match::config::{
    RequestMatcherConversionError, RouteMatchWrapper,
};
use crate::api::v1::http::route::timeout::config::{
    HTTPRouteTimeoutWrapper, TimeoutPoliciesConversionError,
};
use gateway_api::httproutes::HTTPRouteRule;
use thiserror::Error;
use typed_builder::TypedBuilder;
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
    #[error("Filter conversion error at index {0}: {1}")]
    Filter(usize, RuleFilterConversionError),
    #[error("Backend conversion error: {0}")]
    Backend(usize, u32),
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

        let timeouts = rule.timeouts.as_ref().map(HTTPRouteTimeoutWrapper::from);
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
            .backends(vec![])
            .build();

        Ok(rule)
    }
}
