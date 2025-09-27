mod basic;
pub mod headers;
pub mod host_header;
pub mod method;
pub mod path;
pub mod query_params;
mod request;
pub mod scoring;

use self::headers::HeadersMatcher;
use self::method::MethodMatcher;
use self::path::PathMatcher;
use self::query_params::QueryParamsMatcher;
use http::request::Parts;
use scoring::RequestMatcherScorer;
use std::sync::Arc;
use typed_builder::TypedBuilder;

trait Matcher {
    fn matches(&self, score: &RequestMatcherScorer, req: &Parts) -> bool;
}

#[derive(Debug, TypedBuilder)]
pub struct RequestMatcher {
    path_matcher: Option<PathMatcher>,
    method_matcher: Option<MethodMatcher>,
    headers_matcher: Option<HeadersMatcher>,
    query_params_matcher: Option<QueryParamsMatcher>,
}

pub trait RequestMatchDetails {
    fn path_prefix(&self) -> Option<Arc<String>>;
}
