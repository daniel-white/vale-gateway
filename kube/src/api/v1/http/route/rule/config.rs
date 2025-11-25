use crate::api::v1::http::route::backend::config::{HTTPBackendReferenceWrapper, RuleBackendConversionError};
use crate::api::v1::http::route::filter::config::{HTTPRouteFilterWrapper, RuleFilterConversionError};
use crate::api::v1::http::route::r#match::config::{RequestMatcherConversionError, RouteMatchWrapper};
use crate::api::v1::http::route::timeout::config::{HTTPRouteTimeoutWrapper, TimeoutPoliciesConversionError};
use gateway_api::httproutes::HTTPRouteRule;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::policy::timeout::TimeoutPolicies;
use vg_config::http::route::rule::Rule;
use vg_config::http::route::rule::backend::{RuleBackend, WeightedBackendRef};
use vg_config::http::route::rule::filter::RuleFilter;
use vg_config::http::route::rule::matcher::RequestMatcher;
use vg_config::http::route::rule::policy::RulePolicies;

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
    TimeoutPolicies(
        #[from]
        #[source]
        TimeoutPoliciesConversionError,
    ),
    #[error("Matcher conversion error at index {0}: {1}")]
    Matcher(usize, #[source] RequestMatcherConversionError),
    #[error("Filter conversion error at index {0}: {1}")]
    Filter(usize, #[source] RuleFilterConversionError),
    #[error("Backend conversion error: {0}")]
    Backend(usize, #[source] RuleBackendConversionError),
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
                RequestMatcher::try_from(match_).map_err(|err| RuleConversionError::Matcher(idx, err))
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
        let timeouts = timeouts.map(TimeoutPolicies::try_from).transpose()?.unwrap_or_default();
        let policies = RulePolicies::builder()
            .timeouts(timeouts)
            .retries(None) // TODO: Implement retries conversion
            .build();

        let backend_refs = rule
            .backend_refs
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, be)| {
                let be = HTTPBackendReferenceWrapper::builder()
                    .namespace(namespace)
                    .backend_ref(be)
                    .build();
                // Convert RuleBackend to WeightedBackendRef
                let rule_backend = RuleBackend::try_from(be).map_err(|err| RuleConversionError::Backend(idx, err))?;
                Ok::<WeightedBackendRef, RuleConversionError>(
                    WeightedBackendRef::builder()
                        .backend_ref(rule_backend.ref_().clone())
                        .weight(rule_backend.weight())
                        .build(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let rule = Self::builder()
            .name(None) // TODO: add name
            .matchers(matchers)
            .backend_refs(backend_refs)
            .filters(filters)
            .policies(policies)
            .build();

        Ok(rule)
    }
}
