use std::collections::HashMap;
use self::filter::RuleFilter;
use crate::route::rule::filter::RuleFilterConversionError;
use crate::route::rule::matcher::request::{RequestMatcher, RequestMatcherConversionError};
use crate::route::rule::policy::{RulePolicies, RulePoliciesConversionError};
use getset::Getters;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::route::rule::Rule as RuleConfig;
use crate::filter::SharedFilterHandler;

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
    Matcher(usize, #[source] RequestMatcherConversionError),

    #[error("filter at index {0} is invalid: {1}")]
    Filter(usize, #[source] RuleFilterConversionError),

    #[error("policies are invalid: {0}")]
    Policies(
        #[from]
        #[source]
        RulePoliciesConversionError,
    ),
}

impl TryFrom<(&HashMap<SharedFilterRef, SharedFilterHandler>, &RuleConfig)> for Rule {
    type Error = RuleConversionError;

    fn try_from((shared_filter_handlers, rule): (&HashMap<SharedFilterRef, SharedFilterHandler>, &RuleConfig)) -> Result<Self, Self::Error> {
        let matchers = rule
            .matchers()
            .iter()
            .enumerate()
            .map(|(idx, matcher)| {
                RequestMatcher::try_from(matcher)
                    .map_err(|err| RuleConversionError::Matcher(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let filters = rule
            .filters()
            .iter()
            .enumerate()
            .map(|(idx, filter)| {
                RuleFilter::try_from((shared_filter_handlers, filter)).map_err(|err| RuleConversionError::Filter(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let policies = rule.policies().try_into()?;

        let rule = Self::builder()
            .name(rule.name())
            .matchers(matchers)
            .filters(filters)
            .policies(policies)
            .build();

        Ok(rule)
    }
}
