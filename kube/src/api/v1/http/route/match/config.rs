use gateway_api::common::{HeaderMatch, HeaderMatchType};
use gateway_api::httproutes::{HTTPMethodMatch, HTTPRouteRulesMatchesPathType, PathMatch, RouteMatch};
use http::Method;
use http::header::{InvalidHeaderName, InvalidHeaderValue};
use regex::Regex;
use thiserror::Error;
use vg_config::http::route::rule::matcher::{
    HeaderMatcher, HeaderValueMatcher, HeadersMatcher, MethodMatcher, PathMatcher, QueryParamMatcher,
    QueryParamValueMatcher, QueryParamsMatcher, RequestMatcher,
};
use vg_core::internal_wrapper;

#[derive(Debug, Error)]
pub enum MethodMatcherConversionError {
    #[error("Invalid method")]
    Method,
}

#[derive(Debug, Error)]
pub enum PathMatcherConversionError {
    #[error("Invalid path configuration")]
    InvalidConfiguration,
    #[error("Invalid path match regular expression: {0}")]
    RegularExpression(
        #[from]
        #[source]
        regex::Error,
    ),
}

#[derive(Debug, Error)]
pub enum HeaderMatcherConversionError {
    #[error("Invalid header name: {0}")]
    Name(#[from] InvalidHeaderName),
    #[error("Invalid exact header value: {0}")]
    ExactValue(#[from] InvalidHeaderValue),
    #[error("Invalid header value match regular expression: {0}")]
    RegularExpression(
        #[from]
        #[source]
        regex::Error,
    ),
}

#[derive(Debug, Error)]
pub enum QueryParamMatcherConversionError {
    #[error("Invalid query parameter value match regular expression: {0}")]
    RegularExpression(
        #[from]
        #[source]
        regex::Error,
    ),
}

#[derive(Debug, Error)]
pub enum RequestMatcherConversionError {
    #[error("Method matcher conversion error: {0}")]
    Method(
        #[from]
        #[source]
        MethodMatcherConversionError,
    ),
    #[error("Path matcher conversion error: {0}")]
    Path(
        #[from]
        #[source]
        PathMatcherConversionError,
    ),
    #[error("Header matcher conversion error at index {0}: {1}")]
    Header(usize, #[source] HeaderMatcherConversionError),
    #[error("Query parameter matcher conversion error at index {0}: {1}")]
    QueryParam(usize, #[source] QueryParamMatcherConversionError),
}

internal_wrapper!(HTTPMethodMatch);

impl TryFrom<HTTPMethodMatchWrapper<'_>> for MethodMatcher {
    type Error = MethodMatcherConversionError;

    fn try_from(method: HTTPMethodMatchWrapper) -> Result<Self, Self::Error> {
        let method = match *method {
            HTTPMethodMatch::Get => Method::GET,
            HTTPMethodMatch::Head => Method::HEAD,
            HTTPMethodMatch::Post => Method::POST,
            HTTPMethodMatch::Put => Method::PUT,
            HTTPMethodMatch::Delete => Method::DELETE,
            HTTPMethodMatch::Connect => Method::CONNECT,
            HTTPMethodMatch::Options => Method::OPTIONS,
            HTTPMethodMatch::Trace => Method::TRACE,
            HTTPMethodMatch::Patch => Method::PATCH,
        };

        let matcher = Self::builder().method(method).build();

        Ok(matcher)
    }
}

internal_wrapper!(PathMatch);

impl TryFrom<PathMatchWrapper<'_>> for PathMatcher {
    type Error = PathMatcherConversionError;

    fn try_from(path: PathMatchWrapper) -> Result<Self, Self::Error> {
        match (&path.r#type, &path.value) {
            (Some(r#type), Some(path_value)) => match r#type {
                HTTPRouteRulesMatchesPathType::Exact => Ok(Self::Exact(path_value.clone())),
                HTTPRouteRulesMatchesPathType::PathPrefix => Ok(Self::Prefix(path_value.clone())),
                HTTPRouteRulesMatchesPathType::RegularExpression => Regex::new(path_value)
                    .map(|_| Self::RegularExpression(path_value.clone()))
                    .map_err(PathMatcherConversionError::RegularExpression),
            },
            _ => Err(PathMatcherConversionError::InvalidConfiguration),
        }
    }
}

internal_wrapper!(HeaderMatch);

impl TryFrom<HeaderMatchWrapper<'_>> for HeaderMatcher {
    type Error = HeaderMatcherConversionError;

    fn try_from(header: HeaderMatchWrapper) -> Result<Self, Self::Error> {
        let name = header.name.parse()?;

        let value = match &header.r#type {
            None | Some(HeaderMatchType::Exact) => header
                .value
                .parse()
                .map(HeaderValueMatcher::Exact)
                .map_err(HeaderMatcherConversionError::ExactValue)?,
            Some(HeaderMatchType::RegularExpression) => Regex::new(&header.value)
                .map(|_| HeaderValueMatcher::RegularExpression(header.value.clone()))
                .map_err(HeaderMatcherConversionError::RegularExpression)?,
        };

        let matcher = Self::builder().name(name).value(value).build();

        Ok(matcher)
    }
}

type QueryParamMatch = HeaderMatch;
internal_wrapper!(QueryParamMatch);

impl TryFrom<QueryParamMatchWrapper<'_>> for QueryParamMatcher {
    type Error = QueryParamMatcherConversionError;

    fn try_from(param: QueryParamMatchWrapper) -> Result<Self, Self::Error> {
        let value = match &param.r#type {
            None | Some(HeaderMatchType::Exact) => QueryParamValueMatcher::Exact(param.value.clone()),
            Some(HeaderMatchType::RegularExpression) => {
                Regex::new(&param.value).map(|_| QueryParamValueMatcher::RegularExpression(param.value.clone()))?
            }
        };

        let matcher = Self::builder().name(param.name.clone()).value(value).build();

        Ok(matcher)
    }
}

type HeaderMatches = Vec<HeaderMatch>;
internal_wrapper!(HeaderMatches);

impl TryFrom<HeaderMatchesWrapper<'_>> for HeadersMatcher {
    type Error = RequestMatcherConversionError;

    fn try_from(headers: HeaderMatchesWrapper) -> Result<Self, Self::Error> {
        let headers = headers
            .iter()
            .map(HeaderMatchWrapper)
            .enumerate()
            .map(|(idx, header)| {
                HeaderMatcher::try_from(header).map_err(|err| RequestMatcherConversionError::Header(idx, err))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let matcher = Self::builder().headers(headers).build();

        Ok(matcher)
    }
}

type QueryParamMatches = Vec<HeaderMatch>;
internal_wrapper!(QueryParamMatches);

impl TryFrom<QueryParamMatchesWrapper<'_>> for QueryParamsMatcher {
    type Error = RequestMatcherConversionError;

    fn try_from(query_params: QueryParamMatchesWrapper) -> Result<Self, Self::Error> {
        let query_params = query_params
            .iter()
            .map(QueryParamMatchWrapper)
            .enumerate()
            .map(|(idx, query_param)| {
                QueryParamMatcher::try_from(query_param)
                    .map_err(|err| RequestMatcherConversionError::QueryParam(idx, err))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let matcher = Self::builder().query_params(query_params).build();

        Ok(matcher)
    }
}

internal_wrapper!(RouteMatch);

impl TryFrom<RouteMatchWrapper<'_>> for RequestMatcher {
    type Error = RequestMatcherConversionError;

    fn try_from(value: RouteMatchWrapper) -> Result<Self, Self::Error> {
        let method = value
            .method
            .as_ref()
            .map(HTTPMethodMatchWrapper)
            .map(MethodMatcher::try_from)
            .transpose()?;

        let path = value
            .path
            .as_ref()
            .map(PathMatchWrapper)
            .map(PathMatcher::try_from)
            .transpose()?;

        let headers = value
            .headers
            .as_ref()
            .map(HeaderMatchesWrapper)
            .map(HeadersMatcher::try_from)
            .transpose()?
            .unwrap_or_default();

        let query_params = value
            .query_params
            .as_ref()
            .map(QueryParamMatchesWrapper)
            .map(QueryParamsMatcher::try_from)
            .transpose()?
            .unwrap_or_default();

        Ok(RequestMatcher::builder()
            .method(method)
            .path(path)
            .headers(headers)
            .query_params(query_params)
            .build())
    }
}
