use crate::routing::rule::matcher::request::{RequestMatcher, RequestMatcherConversionError};
use crate::routing::rule::policy::{RulePolicies, RulePoliciesConversionError};
use getset::Getters;
use thiserror::Error;
use typed_builder::TypedBuilder;

pub mod matcher;
pub mod policy;

#[derive(Debug, TypedBuilder, Getters)]
pub struct Rule {
    #[getset(get = "pub")]
    name: Option<String>,

    #[getset(get = "pub")]
    matcher: RequestMatcher,

    #[getset(get = "pub")]
    filters: Vec<u32>,

    #[getset(get = "pub")]
    policies: RulePolicies,
}

#[derive(Debug, Error)]
pub enum RuleConversionError {
    #[error("request matcher is invalid: {0}")]
    Matcher(#[from] RequestMatcherConversionError),

    #[error("filter at index {0} is invalid: {1}")]
    Filter(usize, u32),

    #[error("policies are invalid: {0}")]
    Policies(#[from] RulePoliciesConversionError),
}

impl TryFrom<&vg_http_config::routing::rule::Rule> for Rule {
    type Error = RuleConversionError;

    fn try_from(value: &vg_http_config::routing::rule::Rule) -> Result<Self, Self::Error> {
        let matcher = value.matcher().try_into()?;

        // let filters = value.filters().iter().enumerate().map(|(idx, filter)| {
        //     // Here we would normally validate the filter.
        //     // For this example, we assume all u32 filters are valid.
        //     Ok(*filter)
        // }).collect::<Result<Vec<_>, _>>().map_err(|err| RuleConversionError::Filter(e.0, e.1))?;

        let policies = value.policies().try_into()?;

        let rule = Self::builder()
            .name(value.name())
            .matcher(matcher)
            .filters(vec![])
            .policies(policies)
            .build();

        Ok(rule)
    }
}
