use super::Matcher;
use super::RequestMatchDetails;
use super::header::{HeadersMatcher, HeadersMatcherConversionError};
use super::method::{MethodMatcher, MethodMatcherConversionError};
use super::path::{PathMatcher, PathMatcherConversionError};
use super::query_param::{QueryParamsMatcher, QueryParamsMatcherConversionError};
use super::scoring::{RequestMatchScore, RequestMatcherScorer};
use http::request::Parts;
use thiserror::Error;
use tracing::{debug, instrument, trace};
use typed_builder::TypedBuilder;
use vg_http_config::routing::rule::matcher::RequestMatcher as RequestMatcherConfig;

#[derive(Debug, TypedBuilder)]
pub struct RequestMatcher {
    #[builder(setter(into))]
    method: Option<MethodMatcher>,
    #[builder(setter(into))]
    path: Option<PathMatcher>,
    #[builder(setter(into))]
    headers: Option<HeadersMatcher>,
    #[builder(setter(into))]
    query_params: Option<QueryParamsMatcher>,
}

impl RequestMatcher {
    #[instrument(skip(self, req), name = "RequestMatcher::matches")]
    pub fn matches(&self, req: &Parts) -> RequestMatcherResult {
        let scorer = RequestMatcherScorer::default();

        if let Some(method) = &self.method {
            trace!("Testing method for match");
            if !method.matches(&scorer, req) {
                debug!("Method did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        if let Some(path) = &self.path {
            trace!("Testing path for match");
            if !path.matches(&scorer, req) {
                debug!("Path did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        if let Some(headers) = &self.headers {
            trace!("Testing headers for match");
            if !headers.matches(&scorer, req) {
                debug!("Headers did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        if let Some(query_params) = &self.query_params {
            trace!("Testing query parameters for match");
            if !query_params.matches(&scorer, req) {
                debug!("Query parameters did not match");
                return RequestMatcherResult::NotMatched;
            }
        }

        debug!("All route rule matches succeeded");
        let score = scorer.results();
        RequestMatcherResult::Matched(score)
    }
}

#[derive(Debug, Error)]
pub enum RequestMatcherConversionError {
    #[error("Invalid method matcher: {0}")]
    InvalidMethodMatcher(#[from] MethodMatcherConversionError),
    #[error("Invalid path matcher: {0}")]
    InvalidPathMatcher(#[from] PathMatcherConversionError),
    #[error("Invalid headers matcher: {0}")]
    InvalidHeadersMatcher(#[from] HeadersMatcherConversionError),
    #[error("Invalid query params matcher: {0}")]
    InvalidQueryParamsMatcher(#[from] QueryParamsMatcherConversionError),
}

impl TryFrom<&RequestMatcherConfig> for RequestMatcher {
    type Error = RequestMatcherConversionError;

    fn try_from(value: &RequestMatcherConfig) -> Result<Self, Self::Error> {
        let method = value
            .method()
            .as_ref()
            .map(MethodMatcher::try_from)
            .transpose()?;
        let path = value
            .path()
            .as_ref()
            .map(PathMatcher::try_from)
            .transpose()?;
        let headers: HeadersMatcher = value.headers().try_into()?;
        let query_params: QueryParamsMatcher = value.query_params().try_into()?;

        let matcher = Self::builder()
            .method(method)
            .path(path)
            .headers(headers)
            .query_params(query_params)
            .build();

        Ok(matcher)
    }
}

/// Enhanced result that includes matched prefix context
#[derive(Debug, PartialEq, Eq)]
pub enum RequestMatcherResult {
    Matched(RequestMatchScore),
    NotMatched,
}

impl RequestMatcherResult {
    pub fn is_matched(&self) -> bool {
        matches!(self, Self::Matched { .. })
    }

    pub fn score(&self) -> Option<&RequestMatchScore> {
        match self {
            Self::Matched(score) => Some(score),
            Self::NotMatched => None,
        }
    }
}

impl RequestMatchDetails for RequestMatcherResult {
    fn path_prefix(&self) -> Option<String> {
        self.score().and_then(|s| s.path_prefix())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::rule::matcher::header::HeaderMatcher;
    use crate::routing::rule::matcher::query_param::QueryParamMatcher;
    use assertables::*;
    use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
    use http::{HeaderValue, Method, Request, Version};
    use regex::Regex;
    use rstest::*;

    // Helper function to create route parts
    fn create_request_parts(method: Method, uri: &str) -> Parts {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .version(Version::HTTP_11)
            .body(())
            .unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    fn create_request_parts_with_headers(
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
    ) -> Parts {
        let mut request_builder = Request::builder()
            .method(method)
            .uri(uri)
            .version(Version::HTTP_11);

        for (key, value) in headers {
            request_builder = request_builder.header(*key, *value);
        }

        let request = request_builder.body(()).unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    // Fixtures for different matcher types
    #[fixture]
    fn path_exact_matcher() -> PathMatcher {
        PathMatcher::Exact("/api/v1/test".into())
    }

    #[fixture]
    fn path_prefix_matcher() -> PathMatcher {
        PathMatcher::Prefix("/api".into())
    }

    #[fixture]
    fn path_regex_matcher() -> PathMatcher {
        let regex = Regex::new(r"^/api/v[0-9]+/.*$").unwrap();
        PathMatcher::RegularExpression(regex.into())
    }

    #[fixture]
    fn headers_matcher_single() -> HeadersMatcher {
        let header_value = HeaderValue::from_static("application/json");
        let header_matcher = HeaderMatcher::new_exact(CONTENT_TYPE, header_value);
        HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build()
    }

    #[fixture]
    fn headers_matcher_multiple() -> HeadersMatcher {
        let header_value = HeaderValue::from_static("application/json");
        let header1 = HeaderMatcher::new_exact(CONTENT_TYPE, header_value.clone());
        let header2 = HeaderMatcher::new_exact(ACCEPT, header_value);
        HeadersMatcher::builder()
            .matchers(vec![header1, header2])
            .build()
    }

    #[fixture]
    fn query_params_matcher() -> QueryParamsMatcher {
        let param_matcher = QueryParamMatcher::new_exact("version", "v1");
        QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build()
    }

    // Tests for matching behavior with no matcher (should always match)
    #[rstest]
    fn test_no_matchers_always_matches() {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/any/path");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert!(result.score().is_some());
    }

    // Tests for method matching
    #[rstest]
    fn test_method_matcher_success() {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/test");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_method_matcher_failure() {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(Some(MethodMatcher::from(Method::PATCH)))
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::POST, "http://example.com/test");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
        assert!(!result.is_matched());
        assert_none!(result.score());
    }

    // Tests for path matching
    #[rstest]
    fn test_path_exact_matcher_success(path_exact_matcher: PathMatcher) {
        let matcher = RequestMatcher::builder()
            .path(Some(path_exact_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/api/v1/test");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_path_exact_matcher_failure(path_exact_matcher: PathMatcher) {
        let matcher = RequestMatcher::builder()
            .path(Some(path_exact_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/api/v2/test");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
        assert!(!result.is_matched());
    }

    #[rstest]
    fn test_path_prefix_matcher_success(path_prefix_matcher: PathMatcher) {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/api/v1/users");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some_eq_x!(result.path_prefix(), "/api");
    }

    #[rstest]
    fn test_path_prefix_matcher_failure(path_prefix_matcher: PathMatcher) {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/web/v1/users");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_path_regex_matcher_success(path_regex_matcher: PathMatcher) {
        let matcher = RequestMatcher::builder()
            .path(Some(path_regex_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/api/v2/users");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_none!(result.path_prefix());
    }

    #[rstest]
    fn test_path_regex_matcher_failure(path_regex_matcher: PathMatcher) {
        let matcher = RequestMatcher::builder()
            .path(Some(path_regex_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/web/v1/users");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    // Tests for header matching
    #[rstest]
    fn test_headers_matcher_success(headers_matcher_single: HeadersMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(Some(headers_matcher_single))
            .query_params(None)
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/test",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
    }

    #[rstest]
    fn test_headers_matcher_failure(headers_matcher_single: HeadersMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(Some(headers_matcher_single))
            .query_params(None)
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/test",
            &[("content-type", "text/plain")],
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_headers_matcher_missing_header(headers_matcher_single: HeadersMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(Some(headers_matcher_single))
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/test");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_headers_matcher_multiple_success(headers_matcher_multiple: HeadersMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(Some(headers_matcher_multiple))
            .query_params(None)
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/test",
            &[
                ("content-type", "application/json"),
                ("accept", "application/json"),
            ],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_headers_matcher_multiple_partial_failure(headers_matcher_multiple: HeadersMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(Some(headers_matcher_multiple))
            .query_params(None)
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/test",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    // Tests for query parameter matching
    #[rstest]
    fn test_query_params_matcher_success(query_params_matcher: QueryParamsMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(None)
            .query_params(Some(query_params_matcher))
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/test?version=v1");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_query_params_matcher_failure(query_params_matcher: QueryParamsMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(None)
            .query_params(Some(query_params_matcher))
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/test?version=v2");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_query_params_matcher_missing_param(query_params_matcher: QueryParamsMatcher) {
        let matcher = RequestMatcher::builder()
            .path(None)
            .method(None)
            .headers(None)
            .query_params(Some(query_params_matcher))
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/test");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    // Tests for combined matcher (all must match)
    #[rstest]
    fn test_all_matchers_success() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/api/v1/users?version=v1",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_all_matchers_method_failure() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::POST, // Wrong method
            "http://example.com/api/v1/users?version=v1",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_all_matchers_path_failure() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/web/v1/users?version=v1", // Wrong path
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_all_matchers_headers_failure() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/api/v1/users?version=v1",
            &[("content-type", "text/plain")], // Wrong header value
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_all_matchers_query_params_failure() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/api/v1/users?version=v2", // Wrong query param value
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    // Tests for RequestMatcherResult methods
    #[rstest]
    fn test_request_matcher_result_is_matched() {
        let score = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        let matched_result = RequestMatcherResult::Matched(score);
        let not_matched_result = RequestMatcherResult::NotMatched;

        assert!(matched_result.is_matched());
        assert!(!not_matched_result.is_matched());
    }

    #[rstest]
    fn test_request_matcher_result_score() {
        let score = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        let matched_result = RequestMatcherResult::Matched(score);
        let not_matched_result = RequestMatcherResult::NotMatched;

        assert_some!(matched_result.score());
        assert_none!(not_matched_result.score());
    }

    #[rstest]
    fn test_request_matcher_result_path_prefix() {
        let score = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(8))
            .path_prefix(Some("/api/v1".to_string()))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let matched_result = RequestMatcherResult::Matched(score);
        let not_matched_result = RequestMatcherResult::NotMatched;

        assert_some_eq_x!(matched_result.path_prefix(), "/api/v1");
        assert_none!(not_matched_result.path_prefix());
    }

    // Tests for short-circuit behavior (first failure stops evaluation)
    #[rstest]
    fn test_short_circuit_on_method_failure() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_exact_matcher())) // This would match
            .method(Some(MethodMatcher::from(Method::GET))) // This will fail
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::POST, // Wrong method - should fail early
            "http://example.com/api/v1/test?version=v1",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    // Tests for edge cases
    #[rstest]
    fn test_empty_uri_path() {
        let path_matcher = PathMatcher::Exact("".into());
        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com");
        let result = matcher.matches(&parts);

        assert_eq!(result, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_root_path() {
        let path_matcher = PathMatcher::Exact("/".into());
        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(Method::GET, "http://example.com/");
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
    }

    #[rstest]
    fn test_complex_uri_with_query_and_fragment() {
        let path_matcher = PathMatcher::Prefix("/api".into());
        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(
            Method::GET,
            "http://example.com/api/users?page=1&limit=10#section",
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
        assert_eq!(result.path_prefix().unwrap().as_str(), "/api");
    }

    // Tests for case sensitivity and special characters
    #[rstest]
    fn test_case_sensitive_path_matching() {
        let path_matcher = PathMatcher::Exact("/API/Test".into());
        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts_exact = create_request_parts(Method::GET, "http://example.com/API/Test");
        let parts_lowercase = create_request_parts(Method::GET, "http://example.com/api/test");

        let result_exact = matcher.matches(&parts_exact);
        let result_lowercase = matcher.matches(&parts_lowercase);

        assert!(result_exact.is_matched());
        assert_eq!(result_lowercase, RequestMatcherResult::NotMatched);
    }

    #[rstest]
    fn test_special_characters_in_path() {
        let path_matcher = PathMatcher::Exact("/api/users/user-123_test.json".into());
        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let parts = create_request_parts(
            Method::GET,
            "http://example.com/api/users/user-123_test.json",
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
    }

    // Tests for comprehensive scoring scenarios
    #[rstest]
    fn test_comprehensive_scoring_exact_path() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_exact_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_multiple()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/api/v1/test?version=v1",
            &[
                ("content-type", "application/json"),
                ("accept", "application/json"),
            ],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_comprehensive_scoring_prefix_path() {
        let matcher = RequestMatcher::builder()
            .path(Some(path_prefix_matcher()))
            .method(Some(MethodMatcher::from(Method::GET)))
            .headers(Some(headers_matcher_single()))
            .query_params(Some(query_params_matcher()))
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://example.com/api/v2/extended/path?version=v1&extra=param",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
        assert_some_eq_x!(result.path_prefix(), "/api");
    }

    // Performance and stress tests
    #[rstest]
    fn test_complex_regex_pattern_performance() {
        let complex_regex = Regex::new(r"^/api/v[0-9]+/(users|posts|comments)/[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}(/edit|/view)?$").unwrap();
        let path_matcher = PathMatcher::RegularExpression(complex_regex.into());

        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(None)
            .headers(None)
            .query_params(None)
            .build();

        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let valid_path = format!("/api/v1/users/{}/edit", uuid);
        let parts = create_request_parts(Method::GET, &format!("http://example.com{}", valid_path));

        let result = matcher.matches(&parts);
        assert!(result.is_matched());
    }

    // Integration tests with real-world scenarios
    #[rstest]
    fn test_rest_api_endpoint_matching() {
        // Simulate a typical REST API endpoint matcher
        let regex = Regex::new(r"^/api/v[0-9]+/users/[0-9]+$").unwrap();
        let path_matcher = PathMatcher::RegularExpression(regex.into());
        let method_matcher: MethodMatcher = Method::GET.into();
        let bearer_token = HeaderValue::from_static("Bearer token123");
        let auth_header = HeaderMatcher::new_exact(AUTHORIZATION, bearer_token);
        let headers_matcher = HeadersMatcher::builder()
            .matchers(vec![auth_header])
            .build();

        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(Some(method_matcher))
            .headers(Some(headers_matcher))
            .query_params(None)
            .build();

        let parts = create_request_parts_with_headers(
            Method::GET,
            "http://api.example.com/api/v2/users/12345",
            &[("authorization", "Bearer token123")],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }

    #[rstest]
    fn test_graphql_endpoint_matching() {
        // Simulate a GraphQL endpoint matcher
        let path_matcher = PathMatcher::Exact("/graphql".into());
        let method_matcher: MethodMatcher = Method::POST.into();
        let content_type = HeaderValue::from_static("application/json");
        let content_type_header = HeaderMatcher::new_exact(CONTENT_TYPE, content_type);
        let headers_matcher = HeadersMatcher::builder()
            .matchers(vec![content_type_header])
            .build();

        let matcher = RequestMatcher::builder()
            .path(Some(path_matcher))
            .method(Some(method_matcher))
            .headers(Some(headers_matcher))
            .query_params(None)
            .build();

        let parts = create_request_parts_with_headers(
            Method::POST,
            "http://api.example.com/graphql",
            &[("content-type", "application/json")],
        );
        let result = matcher.matches(&parts);

        assert!(result.is_matched());
        assert_some!(result.score());
    }
}
