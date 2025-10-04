use self::filter::RuleFilter;
use crate::routing::rule::filter::RuleFilterConversionError;
use crate::routing::rule::matcher::request::{RequestMatcher, RequestMatcherConversionError};
use crate::routing::rule::policy::{RulePolicies, RulePoliciesConversionError};
use getset::Getters;
use thiserror::Error;
use typed_builder::TypedBuilder;

pub mod filter;
pub mod matcher;
pub mod policy;

#[derive(Debug, TypedBuilder, Getters)]
pub struct Rule {
    #[getset(get = "pub")]
    name: Option<String>,

    #[getset(get = "pub")]
    matchers: Vec<RequestMatcher>,

    #[getset(get = "pub")]
    filters: Vec<RuleFilter>,

    #[getset(get = "pub")]
    policies: RulePolicies,
}

#[derive(Debug, Error)]
pub enum RuleConversionError {
    #[error("request matcher at index {0} is invalid: {1}")]
    Matcher(usize, RequestMatcherConversionError),

    #[error("filter at index {0} is invalid: {1}")]
    Filter(usize, RuleFilterConversionError),

    #[error("policies are invalid: {0}")]
    Policies(#[from] RulePoliciesConversionError),
}

impl TryFrom<&vg_http_config::routing::rule::Rule> for Rule {
    type Error = RuleConversionError;

    fn try_from(value: &vg_http_config::routing::rule::Rule) -> Result<Self, Self::Error> {
        let matchers = value
            .matchers()
            .iter()
            .enumerate()
            .map(|(idx, matcher)| {
                RequestMatcher::try_from(matcher)
                    .map_err(|err| RuleConversionError::Matcher(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let filters = value
            .filters()
            .iter()
            .enumerate()
            .map(|(idx, filter)| {
                RuleFilter::try_from(filter).map_err(|err| RuleConversionError::Filter(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let policies = value.policies().try_into()?;

        let rule = Self::builder()
            .name(value.name())
            .matchers(matchers)
            .filters(filters)
            .policies(policies)
            .build();

        Ok(rule)
    }
}
