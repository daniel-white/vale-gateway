use crate::api::v1::http::routing::r#match::RouteMatch;
use gateway_api::common::HeaderMatchType;
use gateway_api::httproutes::{HTTPMethodMatch, HTTPRouteRulesMatchesPathType};
use http::Method;
use http::header::{InvalidHeaderName, InvalidHeaderValue};
use regex::Regex;
use thiserror::Error;
use vg_http_config::request::matchers::{
    HeaderMatcher, HeaderValueMatcher, HeadersMatcher, MethodMatcher, PathMatcher,
    QueryParamMatcher, QueryParamValueMatcher, QueryParamsMatcher, RequestMatcher,
};

#[derive(Debug, Error)]
pub enum RequestMatcherConversionError {
    #[error("Invalid configuration")]
    InvalidConfiguration,
    #[error("Invalid method")]
    Method,
    #[error("Invalid path configuration")]
    Path,
    #[error("Invalid path match regular expression: {0}")]
    PathRegularExpression(regex::Error),
    #[error("Invalid header configuration at index {0}")]
    Header(usize),
    #[error("Invalid header name at index {0}: {1}")]
    HeaderName(usize, InvalidHeaderName),
    #[error("Invalid exact header value at index {0}: {1}")]
    ExactHeaderValue(usize, InvalidHeaderValue),
    #[error("Invalid header value match regular expression at index {0}: {1}")]
    HeaderValueRegularExpression(usize, regex::Error),
    #[error("Invalid query parameter configuration at index {0}")]
    QueryParam(usize),
    #[error("Invalid query parameter value match regular expression at index {0}: {1}")]
    QueryParamValueRegularExpression(usize, regex::Error),
}

impl TryFrom<&RouteMatch> for RequestMatcher {
    type Error = RequestMatcherConversionError;

    fn try_from(value: &RouteMatch) -> Result<Self, Self::Error> {
        let method_matcher = value
            .method
            .as_ref()
            .map(|method| match method {
                HTTPMethodMatch::Get => Ok(Method::GET),
                HTTPMethodMatch::Head => Ok(Method::HEAD),
                HTTPMethodMatch::Post => Ok(Method::POST),
                HTTPMethodMatch::Put => Ok(Method::PUT),
                HTTPMethodMatch::Delete => Ok(Method::DELETE),
                HTTPMethodMatch::Connect => Ok(Method::CONNECT),
                HTTPMethodMatch::Options => Ok(Method::OPTIONS),
                HTTPMethodMatch::Trace => Ok(Method::TRACE),
                HTTPMethodMatch::Patch => Ok(Method::PATCH),
            })
            .transpose()?
            .map(|m| MethodMatcher::builder().method(m).build());

        let path_matcher = value
            .path
            .as_ref()
            .map(|path| match (&path.r#type, &path.value) {
                (Some(r#type), Some(value)) => Ok((r#type, value)),
                _ => Err(RequestMatcherConversionError::Path),
            })
            .transpose()?
            .map(|(r#type, value)| match r#type {
                HTTPRouteRulesMatchesPathType::Exact => Ok(PathMatcher::Exact(value.clone())),
                HTTPRouteRulesMatchesPathType::PathPrefix => Ok(PathMatcher::Prefix(value.clone())),
                HTTPRouteRulesMatchesPathType::RegularExpression => Regex::new(value)
                    .map(|_| PathMatcher::RegularExpression(value.clone()))
                    .map_err(RequestMatcherConversionError::PathRegularExpression),
            })
            .transpose()?;

        let headers_matcher = value
            .headers
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, header_match)| {
                let name = header_match
                    .name
                    .parse()
                    .map_err(|err| RequestMatcherConversionError::HeaderName(idx, err))?;

                let value = match &header_match.r#type {
                    None | Some(HeaderMatchType::Exact) => header_match
                        .value
                        .parse()
                        .map(|header_value: http::HeaderValue| {
                            HeaderValueMatcher::Exact(header_value)
                        })
                        .map_err(|err| RequestMatcherConversionError::ExactHeaderValue(idx, err)),
                    Some(HeaderMatchType::RegularExpression) => Regex::new(&header_match.value)
                        .map(|_| HeaderValueMatcher::RegularExpression(header_match.value.clone()))
                        .map_err(|err| {
                            RequestMatcherConversionError::HeaderValueRegularExpression(idx, err)
                        }),
                }?;

                let matcher = HeaderMatcher::builder().name(name).value(value).build();

                Ok(matcher)
            })
            .collect::<Result<_, _>>()
            .map(|matchers| HeadersMatcher::builder().headers(matchers).build())?;

        let query_params_matcher = value
            .query_params
            .iter()
            .flatten()
            .enumerate()
            .map(|(idx, param_match)| {
                let value = match &param_match.r#type {
                    None | Some(HeaderMatchType::Exact) => {
                        QueryParamValueMatcher::Exact(param_match.value.clone())
                    }
                    Some(HeaderMatchType::RegularExpression) => Regex::new(&param_match.value)
                        .map(|_| {
                            QueryParamValueMatcher::RegularExpression(param_match.value.clone())
                        })
                        .map_err(|err| {
                            RequestMatcherConversionError::QueryParamValueRegularExpression(
                                idx, err,
                            )
                        })?,
                };

                let matcher = QueryParamMatcher::builder()
                    .name(param_match.name.clone())
                    .value(value)
                    .build();

                Ok(matcher)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|matchers| QueryParamsMatcher::builder().query_params(matchers).build())?;

        let matcher = RequestMatcher::builder()
            .method(method_matcher)
            .path(path_matcher)
            .headers(headers_matcher)
            .query_params(query_params_matcher)
            .build();

        Ok(matcher)
    }
}
