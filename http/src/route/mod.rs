use std::collections::HashMap;
use crate::route::host::{HostMatcher, HostMatcherConversionError};
use crate::route::rule::{Rule, RuleConversionError};
use getset::{CloneGetters, Getters};
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_config::http::filter::SharedFilterRef;
use vg_config::http::route::{Route as RouteConfig, RouteRef};
use crate::filter::SharedFilterHandler;

pub mod host;
pub mod rule;

#[derive(Debug, TypedBuilder, Getters, CloneGetters)]
pub struct Route {
    #[getset(get_clone = "pub")]
    ref_: RouteRef,
    #[getset(get = "pub")]
    host_matchers: Vec<HostMatcher>,
    #[getset(get = "pub")]
    rules: Vec<Rule>,
}

#[derive(Debug, Error)]
pub enum RouteConversionError {
    #[error("Host matcher at index {0} is invalid: {1}")]
    HostMatcher(usize, #[source] HostMatcherConversionError),

    #[error("Rule at index {0} is invalid: {1}")]
    Rule(usize, #[source] RuleConversionError),
}

impl TryFrom<(&HashMap<SharedFilterRef, SharedFilterHandler>, &RouteConfig)> for Route {
    type Error = RouteConversionError;

    fn try_from((shared_filter_handlers, route): (&HashMap<SharedFilterRef, SharedFilterHandler>, &RouteConfig)) -> Result<Self, Self::Error> {
        let host_matchers = route
            .host_matchers()
            .iter()
            .enumerate()
            .map(|(idx, matcher)| {
                HostMatcher::try_from(matcher)
                    .map_err(|err| RouteConversionError::HostMatcher(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let rules = route
            .rules()
            .iter()
            .enumerate()
            .map(|(idx, rule)| {
                Rule::try_from((shared_filter_handlers, rule)).map_err(|err| RouteConversionError::Rule(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let route = Self::builder()
            .ref_(route.ref_().clone())
            .host_matchers(host_matchers)
            .rules(rules)
            .build();

        Ok(route)
    }
}
