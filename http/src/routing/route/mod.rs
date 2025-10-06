use std::sync::Arc;
use getset::Getters;
use thiserror::Error;
use typed_builder::TypedBuilder;
use crate::routing::route::host::{HostMatcher, HostMatcherConversionError};
use crate::routing::rule::{Rule, RuleConversionError};
use vg_http_config::routing::route::Route as RouteConfig;

pub mod host;

#[derive(Debug, TypedBuilder, Getters)]
pub struct Route {
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    host_matchers: Vec<HostMatcher>,
    #[getset(get = "pub")]
    rules: Vec<Rule>,
}

#[derive(Debug, Error)]
pub enum RouteConversionError {
    #[error("Host matcher at index {0} is invalid: {0}")]
    HostMatcher(usize, HostMatcherConversionError),
    
    #[error("Rule at index {0} is invalid: {1}")]
    Rule(usize, RuleConversionError),
}

impl TryFrom<&RouteConfig> for Route {
    type Error = RouteConversionError;

    fn try_from(value: &RouteConfig) -> Result<Self, Self::Error> {
        let host_matchers = value
            .host_matchers()
            .iter()
            .enumerate()
            .map(|(idx, matcher)| {
                HostMatcher::try_from(matcher)
                    .map_err(|err| RouteConversionError::HostMatcher(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let rules = value
            .rules()
            .iter()
            .enumerate()
            .map(|(idx, rule)| {
                Rule::try_from(rule).map_err(|err| RouteConversionError::Rule(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let route = Self::builder()
            .name(value.name().clone())
            .host_matchers(host_matchers)
            .rules(rules)
            .build();

        Ok(route)
    }
}

