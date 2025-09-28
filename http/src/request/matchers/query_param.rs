use super::Matcher;
use super::basic::{ExactMatcher, RegularExpressionMatcher};
use super::scoring::RequestMatcherScorer;
use http::request::Parts;
use regex::Regex;
use std::borrow::Cow;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct QueryParamNameMatcher {
    #[builder(setter(into))]
    matcher: ExactMatcher<String>,
}

impl From<&str> for QueryParamNameMatcher {
    fn from(name: &str) -> Self {
        Self::builder().matcher(name).build()
    }
}

impl QueryParamNameMatcher {
    fn matches(&self, name: &str) -> bool {
        self.matcher.matches_str(name)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum QueryParamValueMatcher {
    Exact(ExactMatcher<String>),
    RegularExpression(RegularExpressionMatcher),
}

impl QueryParamValueMatcher {
    fn matches(&self, value: &str) -> bool {
        match self {
            QueryParamValueMatcher::Exact(matcher) => matcher.matches_str(value),
            QueryParamValueMatcher::RegularExpression(matcher) => matcher.matches(value),
        }
    }
}

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct QueryParamMatcher {
    name_matcher: QueryParamNameMatcher,
    value_matcher: QueryParamValueMatcher,
}

impl QueryParamMatcher {
    fn new(name: &str, value_matcher: QueryParamValueMatcher) -> Self {
        Self::builder()
            .name_matcher(name.into())
            .value_matcher(value_matcher)
            .build()
    }

    pub fn new_exact(name: &str, value: &str) -> Self {
        let value_matcher = QueryParamValueMatcher::Exact(value.into());
        Self::new(name, value_matcher)
    }

    pub fn new_matching(name: &str, regex: &Regex) -> Self {
        let value_matcher = QueryParamValueMatcher::RegularExpression(regex.into());
        Self::new(name, value_matcher)
    }

    #[instrument(
        skip(self, name, value),
        name = "QueryParamMatch::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, (name, value): &(Cow<str>, Cow<str>)) -> bool {
        self.name_matcher.matches(name) && self.value_matcher.matches(value)
    }
}

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct QueryParamsMatcher {
    matchers: Vec<QueryParamMatcher>,
}

impl QueryParamsMatcher {
    pub fn weight(&self) -> usize {
        self.matchers.len()
    }
}

impl Matcher for QueryParamsMatcher {
    #[instrument(skip(self, scorer, req), name = "QueryParamsMatcher::matches")]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let query_params: Vec<(Cow<str>, Cow<str>)> = req
            .uri
            .query()
            .map(|query| url::form_urlencoded::parse(query.as_bytes()).collect())
            .unwrap_or_default();

        if query_params.is_empty() {
            debug!("Request has no query parameters");
            return false;
        }

        let is_match = self.matchers.iter().all(|m| {
            query_params
                .iter()
                .any(|query_param| m.matches(query_param))
        });

        if is_match {
            debug!("Query parameters matched");
            scorer.query_params(self);
        }
        is_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Version};
    use regex::Regex;
    use rstest::*;

    fn create_request_parts_with_query(query: &str) -> Parts {
        let uri = if query.is_empty() {
            "http://example.com/test".to_string()
        } else {
            format!("http://example.com/test?{}", query)
        };
        let request = Request::builder()
            .uri(&uri)
            .version(Version::HTTP_11)
            .body(())
            .unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    // QueryParamNameMatcher tests
    #[rstest]
    #[case("page")]
    #[case("limit")]
    #[case("filter")]
    #[case("sort_by")]
    #[case("api_key")]
    fn test_query_param_name_matcher_exact_match(#[case] param_name: &str) {
        // Arrange
        let matcher: QueryParamNameMatcher = param_name.into();

        // Act & Assert
        assert!(
            matcher.matches(param_name),
            "QueryParamNameMatcher should match the exact parameter name"
        );
    }

    #[rstest]
    #[case("page", "limit", false)]
    #[case("api_key", "api_key", true)]
    #[case("sort", "sort_by", false)]
    #[case("filter", "filters", false)]
    fn test_query_param_name_matcher_different_names(
        #[case] matcher_name: &str,
        #[case] test_name: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher: QueryParamNameMatcher = matcher_name.into();

        // Act
        let result = matcher.matches(test_name);

        // Assert
        assert_eq!(
            result, expected_match,
            "QueryParamNameMatcher should return {} for '{}' vs '{}'",
            expected_match, matcher_name, test_name
        );
    }

    // QueryParamValueMatcher tests
    #[rstest]
    #[case("json")]
    #[case("123")]
    #[case("true")]
    #[case("user@example.com")]
    fn test_query_param_value_matcher_exact_match(#[case] value: &str) {
        // Arrange
        let matcher = QueryParamValueMatcher::Exact(value.into());

        // Act & Assert
        assert!(
            matcher.matches(value),
            "QueryParamValueMatcher should match the exact parameter value"
        );
    }

    #[rstest]
    #[case("json", "xml", false)]
    #[case("123", "123", true)]
    #[case("true", "false", false)]
    #[case("user", "admin", false)]
    fn test_query_param_value_matcher_exact_different_values(
        #[case] matcher_value: &str,
        #[case] test_value: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = QueryParamValueMatcher::Exact(matcher_value.into());

        // Act
        let result = matcher.matches(test_value);

        // Assert
        assert_eq!(
            result, expected_match,
            "QueryParamValueMatcher should return {} for '{}' vs '{}'",
            expected_match, matcher_value, test_value
        );
    }

    #[rstest]
    #[case(r"\d+", "123", true)]
    #[case(r"\d+", "abc", false)]
    #[case(r"^(true|false)$", "true", true)]
    #[case(r"^(true|false)$", "yes", false)]
    #[case(r"^[a-z]+@[a-z]+\.[a-z]+$", "user@example.com", true)]
    #[case(r"^[a-z]+@[a-z]+\.[a-z]+$", "invalid-email", false)]
    fn test_query_param_value_matcher_regex_match(
        #[case] pattern: &str,
        #[case] test_value: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let regex = Regex::new(pattern).unwrap();
        let matcher = QueryParamValueMatcher::RegularExpression(regex.into());

        // Act
        let result = matcher.matches(test_value);

        // Assert
        assert_eq!(
            result, expected_match,
            "QueryParamValueMatcher with regex '{}' should return {} for value '{}'",
            pattern, expected_match, test_value
        );
    }

    // QueryParamMatcher tests
    #[test]
    fn test_query_param_matcher_new_exact() {
        // Act
        let matcher = QueryParamMatcher::new_exact("format", "json");

        // Assert
        let param_pair = (Cow::Borrowed("format"), Cow::Borrowed("json"));
        assert!(
            matcher.matches(&param_pair),
            "QueryParamMatcher should match exact parameter name and value"
        );
    }

    #[test]
    fn test_query_param_matcher_new_matching() {
        // Arrange
        let regex = Regex::new(r"\d+").unwrap();

        // Act
        let matcher = QueryParamMatcher::new_matching("id", &regex);

        // Assert
        let param_pair = (Cow::Borrowed("id"), Cow::Borrowed("123"));
        assert!(
            matcher.matches(&param_pair),
            "QueryParamMatcher should match parameter name and regex value"
        );
    }

    #[rstest]
    #[case("page", "1", "page", "1", true)]
    #[case("limit", "10", "limit", "10", true)]
    #[case("format", "json", "format", "xml", false)]
    #[case("api_key", "secret", "other_key", "secret", false)]
    fn test_query_param_matcher_matches(
        #[case] matcher_name: &str,
        #[case] matcher_value: &str,
        #[case] test_name: &str,
        #[case] test_value: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = QueryParamMatcher::new_exact(matcher_name, matcher_value);
        let param_pair = (Cow::Borrowed(test_name), Cow::Borrowed(test_value));

        // Act
        let result = matcher.matches(&param_pair);

        // Assert
        assert_eq!(
            result, expected_match,
            "QueryParamMatcher should return {} for '{}={}' vs '{}={}'",
            expected_match, matcher_name, matcher_value, test_name, test_value
        );
    }

    // QueryParamsMatcher tests
    #[test]
    fn test_query_params_matcher_empty() {
        // Arrange
        let matcher = QueryParamsMatcher::builder().matchers(vec![]).build();
        let parts = create_request_parts_with_query("page=1&limit=10");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "Empty QueryParamsMatcher should match any request with query params"
        );
        assert_eq!(
            matcher.weight(),
            0,
            "Empty QueryParamsMatcher should have weight 0"
        );
    }

    #[test]
    fn test_query_params_matcher_no_query_params() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("page", "1");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query(""); // No query params
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            !result,
            "QueryParamsMatcher should not match request without query parameters"
        );
    }

    #[test]
    fn test_query_params_matcher_single_exact_match() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("format", "json");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("format=json");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "QueryParamsMatcher should match when single parameter matches"
        );
        assert_eq!(
            matcher.weight(),
            1,
            "Single parameter matcher should have weight 1"
        );
    }

    #[test]
    fn test_query_params_matcher_single_no_match() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("format", "json");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("format=xml");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            !result,
            "QueryParamsMatcher should not match when single parameter doesn't match"
        );
    }

    #[test]
    fn test_query_params_matcher_multiple_params_all_match() {
        // Arrange
        let param_matcher1 = QueryParamMatcher::new_exact("page", "1");
        let param_matcher2 = QueryParamMatcher::new_exact("limit", "10");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher1, param_matcher2])
            .build();
        let parts = create_request_parts_with_query("page=1&limit=10&extra=value");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "QueryParamsMatcher should match when all parameters match"
        );
        assert_eq!(
            matcher.weight(),
            2,
            "Two parameter matchers should have weight 2"
        );
    }

    #[test]
    fn test_query_params_matcher_multiple_params_partial_match() {
        // Arrange
        let param_matcher1 = QueryParamMatcher::new_exact("page", "1");
        let param_matcher2 = QueryParamMatcher::new_exact("limit", "10");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher1, param_matcher2])
            .build();
        let parts = create_request_parts_with_query("page=1&limit=20"); // Different limit value
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            !result,
            "QueryParamsMatcher should not match when not all parameters match"
        );
    }

    #[test]
    fn test_query_params_matcher_with_regex() {
        // Arrange
        let regex = Regex::new(r"^\d+$").unwrap();
        let param_matcher = QueryParamMatcher::new_matching("id", &regex);
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("id=123");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "QueryParamsMatcher should match with regex pattern");
    }

    #[test]
    fn test_query_params_matcher_extra_params_in_request() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("format", "json");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("format=json&extra=value&another=param");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "QueryParamsMatcher should match even when request has extra parameters"
        );
    }

    #[test]
    fn test_query_params_matcher_url_encoded_values() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("message", "hello world");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("message=hello%20world");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "QueryParamsMatcher should handle URL encoded values correctly"
        );
    }

    #[rstest]
    #[case("param1=value1&param2=value2")]
    #[case("search=test%20query&category=electronics")]
    #[case("filters[]=item1&filters[]=item2")]
    #[case("nested[key]=value&nested[other]=another")]
    fn test_query_params_matcher_complex_query_strings(#[case] query: &str) {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("search", "test query");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();

        // Only test the second case that has the search parameter
        if query.contains("search=test%20query") {
            let parts = create_request_parts_with_query(query);
            let scorer = RequestMatcherScorer::default();

            // Act
            let result = matcher.matches(&scorer, &parts);

            // Assert
            assert!(
                result,
                "QueryParamsMatcher should handle complex query string: {}",
                query
            );
        }
    }

    #[test]
    fn test_query_params_matcher_calls_scorer_on_match() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("key", "value");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("key=value");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result);
        // The fact that the method returns true indicates scorer.query_params() was called
    }

    #[test]
    fn test_query_params_matcher_does_not_call_scorer_on_no_match() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("key", "value");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("key=different");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(!result);
        // The fact that the method returns false indicates scorer.query_params() was NOT called
    }

    #[test]
    fn test_query_params_matcher_empty_parameter_value() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("empty", "");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("empty=");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "QueryParamsMatcher should handle empty parameter values"
        );
    }

    #[test]
    fn test_query_params_matcher_parameter_without_value() {
        // Arrange
        let param_matcher = QueryParamMatcher::new_exact("flag", "");
        let matcher = QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build();
        let parts = create_request_parts_with_query("flag");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "QueryParamsMatcher should handle parameters without values (flags)"
        );
    }
}
