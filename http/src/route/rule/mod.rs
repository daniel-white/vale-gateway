use crate::filter::SharedFilterHandlerLayer;
use crate::route::rule::filter::RuleFilterHandlerLayerError;
use crate::route::rule::matcher::request::{RequestMatcher, RequestMatcherConversionError};
use crate::route::rule::policy::{RulePoliciesConversionError, RulePolicyHandlers};
use getset::Getters;
use std::collections::HashMap;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::route::rule::Rule as RuleConfig;

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
    filters: Vec<()>,

    #[getset(get = "pub")]
    policies: RulePolicyHandlers,
}

#[derive(Debug, Error)]
pub enum RuleConversionError {
    #[error("request matcher at index {0} is invalid: {1}")]
    Matcher(usize, #[source] RequestMatcherConversionError),

    #[error("filter at index {0} is invalid: {1}")]
    Filter(usize, #[source] RuleFilterHandlerLayerError),

    #[error("policies are invalid: {0}")]
    Policies(
        #[from]
        #[source]
        RulePoliciesConversionError,
    ),
}

impl
    TryFrom<(
        &HashMap<SharedFilterRef, SharedFilterHandlerLayer>,
        &RuleConfig,
    )> for Rule
{
    type Error = RuleConversionError;

    fn try_from(
        (shared_filter_handlers, rule): (
            &HashMap<SharedFilterRef, SharedFilterHandlerLayer>,
            &RuleConfig,
        ),
    ) -> Result<Self, Self::Error> {
        let matchers = rule
            .matchers()
            .iter()
            .enumerate()
            .map(|(idx, matcher)| {
                RequestMatcher::try_from(matcher)
                    .map_err(|err| RuleConversionError::Matcher(idx, err))
            })
            .collect::<Result<_, _>>()?;

        // let filters = rule
        //     .filters()
        //     .iter()
        //     .enumerate()
        //     .map(|(idx, filter)| {
        //         RuleFilterHandler::try_from((shared_filter_handlers, filter))
        //             .map_err(|err| RuleConversionError::Filter(idx, err))
        //     })
        //     .collect::<Result<_, _>>()?;

        let policies = rule.policies().try_into()?;

        let rule = Self::builder()
            .name(rule.name())
            .matchers(matchers)
            .filters(Vec::new())
            .policies(policies)
            .build();

        Ok(rule)
    }
}
