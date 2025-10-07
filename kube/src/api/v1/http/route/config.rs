use crate::api::v1::http::route::rule::config::{HTTPRouteRuleWrapper, RuleConversionError};
use crate::resources::{HTTPRouteRef, HTTPRouteWrapper};
use std::ops::Deref;
use thiserror::Error;
use vg_config::http::route::host::{HostMatcher, HostMatcherConversionError};
use vg_config::http::route::rule::Rule;
use vg_config::http::route::{Route, RouteRef};

impl From<HTTPRouteRef> for RouteRef {
    fn from(value: HTTPRouteRef) -> Self {
        value.to_string().into()
    }
}

#[derive(Debug, Error)]
pub enum RouteConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Host matcher conversion error at index {0}: {1}")]
    Hostname(usize, #[source] HostMatcherConversionError),
    #[error("Rule conversion error at index {0}: {1}")]
    Rule(usize, #[source] RuleConversionError),
}

impl TryFrom<HTTPRouteWrapper<'_>> for Route {
    type Error = RouteConversionError;

    fn try_from(value: HTTPRouteWrapper<'_>) -> Result<Self, Self::Error> {
        let ref_ = HTTPRouteRef::from(*value.deref());

        let spec = &value.spec;

        let host_matchers = spec
            .hostnames
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, hostname)| {
                hostname
                    .parse::<HostMatcher>()
                    .map_err(|err| RouteConversionError::Hostname(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let rules = spec
            .rules
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, rule)| {
                let rule = HTTPRouteRuleWrapper::builder()
                    .namespace(ref_.namespace())
                    .rule(rule)
                    .build();
                Rule::try_from(rule).map_err(|err| RouteConversionError::Rule(idx, err))
            })
            .collect::<Result<_, _>>()?;

        let route = Self::builder()
            .ref_(ref_)
            .host_matchers(host_matchers)
            .rules(rules)
            .build();

        Ok(route)
    }
}
