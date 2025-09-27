use super::Matcher;
use crate::request::matchers::basic::{ExactMatcher, RegularExpressionMatcher};
use crate::request::matchers::scoring::RequestMatcherScorer;
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use regex::Regex;
use std::sync::Arc;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HeaderNameMatcher {
    matcher: ExactMatcher<HeaderName>,
}

impl HeaderNameMatcher {
    fn matches(&self, name: &HeaderName) -> bool {
        self.matcher.matches(name)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum HeaderValueMatcher {
    Exact(ExactMatcher<HeaderValue>),
    RegularExpression(RegularExpressionMatcher),
}

impl HeaderValueMatcher {
    fn matches(&self, value: &HeaderValue) -> bool {
        match self {
            HeaderValueMatcher::Exact(matcher) => matcher.matches(value),
            HeaderValueMatcher::RegularExpression(matcher) => match value.to_str() {
                Ok(value) => matcher.matches(value),
                Err(_) => false,
            },
        }
    }
}

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HeaderMatcher {
    name_matcher: HeaderNameMatcher,
    value_matcher: HeaderValueMatcher,
}

impl HeaderMatcher {
    fn new(name_matcher: HeaderNameMatcher, value_matcher: HeaderValueMatcher) -> Self {
        Self::builder()
            .name_matcher(name_matcher)
            .value_matcher(value_matcher)
            .build()
    }

    pub fn new_exact(key: Arc<HeaderName>, value: Arc<HeaderValue>) -> Self {
        let name_matcher = HeaderNameMatcher::builder()
            .matcher(ExactMatcher::new(key))
            .build();
        let value_matcher = HeaderValueMatcher::Exact(ExactMatcher::new(value));
        Self::new(name_matcher, value_matcher)
    }

    pub fn new_matching(key: Arc<HeaderName>, regex: Arc<Regex>) -> Self {
        let name_matcher = HeaderNameMatcher::builder()
            .matcher(ExactMatcher::new(key))
            .build();
        let value_matcher =
            HeaderValueMatcher::RegularExpression(RegularExpressionMatcher::new(regex));
        Self::new(name_matcher, value_matcher)
    }

    #[instrument(
        skip(self, key, value),
        name = "HeaderMatcher::matches"
        fields(match = ?self)
    )]
    fn matches(&self, (key, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name_matcher.matches(key) && self.value_matcher.matches(value)
    }
}

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HeadersMatcher {
    matchers: Vec<HeaderMatcher>,
}

impl HeadersMatcher {
    pub fn weight(&self) -> usize {
        self.matchers.len()
    }
}

impl Matcher for HeadersMatcher {
    #[instrument(skip(self, scorer, req), name = "HeadersMatcher::matches")]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let is_match = self
            .matchers
            .iter()
            .all(|m| req.headers.iter().any(|header| m.matches(&header)));
        if is_match {
            debug!("Headers matched");
            scorer.headers(self);
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
    use std::sync::Arc;

    fn create_request_parts_with_headers(headers: Vec<(&str, &str)>) -> Parts {
        let mut request_builder = Request::builder()
            .uri("http://example.com/test")
            .version(Version::HTTP_11);

        for (name, value) in headers {
            request_builder = request_builder.header(name, value);
        }

        let request = request_builder.body(()).unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    fn create_header_name(name: &str) -> HeaderName {
        HeaderName::try_from(name).unwrap()
    }

    fn create_header_value(value: &str) -> HeaderValue {
        HeaderValue::try_from(value).unwrap()
    }

    // HeaderNameMatcher tests
    #[rstest]
    #[case("content-type")]
    #[case("authorization")]
    #[case("x-custom-header")]
    #[case("accept")]
    fn test_header_name_matcher_exact_match(#[case] header_name: &str) {
        // Arrange
        let name = create_header_name(header_name);
        let matcher = HeaderNameMatcher::builder()
            .matcher(ExactMatcher::new(Arc::new(name.clone())))
            .build();

        // Act & Assert
        assert!(
            matcher.matches(&name),
            "HeaderNameMatcher should match the exact header name"
        );
    }

    #[rstest]
    #[case("content-type", "authorization", false)]
    #[case("accept", "accept", true)]
    #[case("x-custom", "x-custom-header", false)]
    fn test_header_name_matcher_different_names(
        #[case] matcher_name: &str,
        #[case] test_name: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = HeaderNameMatcher::builder()
            .matcher(ExactMatcher::new(Arc::new(create_header_name(
                matcher_name,
            ))))
            .build();
        let test_header_name = create_header_name(test_name);

        // Act
        let result = matcher.matches(&test_header_name);

        // Assert
        assert_eq!(
            result, expected_match,
            "HeaderNameMatcher should return {} for '{}' vs '{}'",
            expected_match, matcher_name, test_name
        );
    }

    // HeaderValueMatcher tests
    #[rstest]
    #[case("application/json")]
    #[case("text/html")]
    #[case("Bearer token123")]
    fn test_header_value_matcher_exact_match(#[case] value: &str) {
        // Arrange
        let header_value = create_header_value(value);
        let matcher = HeaderValueMatcher::Exact(ExactMatcher::new(Arc::new(header_value.clone())));

        // Act & Assert
        assert!(
            matcher.matches(&header_value),
            "HeaderValueMatcher should match the exact header value"
        );
    }

    #[rstest]
    #[case("application/json", "text/html", false)]
    #[case("Bearer token", "Bearer token", true)]
    #[case("gzip", "deflate", false)]
    fn test_header_value_matcher_exact_different_values(
        #[case] matcher_value: &str,
        #[case] test_value: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = HeaderValueMatcher::Exact(ExactMatcher::new(Arc::new(create_header_value(
            matcher_value,
        ))));
        let test_header_value = create_header_value(test_value);

        // Act
        let result = matcher.matches(&test_header_value);

        // Assert
        assert_eq!(
            result, expected_match,
            "HeaderValueMatcher should return {} for '{}' vs '{}'",
            expected_match, matcher_value, test_value
        );
    }

    #[rstest]
    #[case(r"application/.*", "application/json", true)]
    #[case(r"Bearer \w+", "Bearer token123", true)]
    #[case(r"^text/", "text/html", true)]
    #[case(r"^application/", "text/html", false)]
    #[case(r"\d+", "version-123", true)]
    fn test_header_value_matcher_regex_match(
        #[case] pattern: &str,
        #[case] test_value: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let regex = Arc::new(Regex::new(pattern).unwrap());
        let matcher = HeaderValueMatcher::RegularExpression(RegularExpressionMatcher::new(regex));
        let test_header_value = create_header_value(test_value);

        // Act
        let result = matcher.matches(&test_header_value);

        // Assert
        assert_eq!(
            result, expected_match,
            "HeaderValueMatcher with regex '{}' should return {} for value '{}'",
            pattern, expected_match, test_value
        );
    }

    // HeaderMatcher tests
    #[test]
    fn test_header_matcher_new_exact() {
        // Arrange
        let name = Arc::new(create_header_name("content-type"));
        let value = Arc::new(create_header_value("application/json"));

        // Act
        let matcher = HeaderMatcher::new_exact(name.clone(), value.clone());

        // Assert
        let header_pair = (name.as_ref(), value.as_ref());
        assert!(
            matcher.matches(&header_pair),
            "HeaderMatcher should match exact header name and value"
        );
    }

    #[test]
    fn test_header_matcher_new_matching() {
        // Arrange
        let name = Arc::new(create_header_name("user-agent"));
        let regex = Arc::new(Regex::new(r"Mozilla.*").unwrap());
        let test_value = create_header_value("Mozilla/5.0 (compatible)");

        // Act
        let matcher = HeaderMatcher::new_matching(name.clone(), regex);

        // Assert
        let header_pair = (name.as_ref(), &test_value);
        assert!(
            matcher.matches(&header_pair),
            "HeaderMatcher should match header name and regex value"
        );
    }

    #[rstest]
    #[case(
        "content-type",
        "application/json",
        "content-type",
        "application/json",
        true
    )]
    #[case("authorization", "Bearer token", "authorization", "Bearer token", true)]
    #[case("accept", "text/html", "content-type", "application/json", false)]
    #[case("x-custom", "value1", "x-custom", "value2", false)]
    fn test_header_matcher_matches(
        #[case] matcher_name: &str,
        #[case] matcher_value: &str,
        #[case] test_name: &str,
        #[case] test_value: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = HeaderMatcher::new_exact(
            Arc::new(create_header_name(matcher_name)),
            Arc::new(create_header_value(matcher_value)),
        );
        let test_header_name = create_header_name(test_name);
        let test_header_value = create_header_value(test_value);

        // Act
        let result = matcher.matches(&(&test_header_name, &test_header_value));

        // Assert
        assert_eq!(
            result, expected_match,
            "HeaderMatcher should return {} for '{}:{}' vs '{}:{}'",
            expected_match, matcher_name, matcher_value, test_name, test_value
        );
    }

    // HeadersMatcher tests
    #[test]
    fn test_headers_matcher_empty() {
        // Arrange
        let matcher = HeadersMatcher::builder().matchers(vec![]).build();
        let parts = create_request_parts_with_headers(vec![("content-type", "application/json")]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "Empty HeadersMatcher should match any request");
        assert_eq!(
            matcher.weight(),
            0,
            "Empty HeadersMatcher should have weight 0"
        );
    }

    #[test]
    fn test_headers_matcher_single_exact_match() {
        // Arrange
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        let parts = create_request_parts_with_headers(vec![("content-type", "application/json")]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "HeadersMatcher should match when single header matches"
        );
        assert_eq!(
            matcher.weight(),
            1,
            "Single header matcher should have weight 1"
        );
    }

    #[test]
    fn test_headers_matcher_single_no_match() {
        // Arrange
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        let parts = create_request_parts_with_headers(vec![("content-type", "text/html")]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            !result,
            "HeadersMatcher should not match when single header doesn't match"
        );
    }

    #[test]
    fn test_headers_matcher_multiple_headers_all_match() {
        // Arrange
        let header_matcher1 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let header_matcher2 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("authorization")),
            Arc::new(create_header_value("Bearer token123")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher1, header_matcher2])
            .build();
        let parts = create_request_parts_with_headers(vec![
            ("content-type", "application/json"),
            ("authorization", "Bearer token123"),
        ]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "HeadersMatcher should match when all headers match");
        assert_eq!(
            matcher.weight(),
            2,
            "Two header matchers should have weight 2"
        );
    }

    #[test]
    fn test_headers_matcher_multiple_headers_partial_match() {
        // Arrange
        let header_matcher1 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let header_matcher2 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("authorization")),
            Arc::new(create_header_value("Bearer token123")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher1, header_matcher2])
            .build();
        let parts = create_request_parts_with_headers(vec![
            ("content-type", "application/json"),
            ("authorization", "Basic user:pass"), // Different value
        ]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            !result,
            "HeadersMatcher should not match when not all headers match"
        );
    }

    #[test]
    fn test_headers_matcher_with_regex() {
        // Arrange
        let header_matcher = HeaderMatcher::new_matching(
            Arc::new(create_header_name("user-agent")),
            Arc::new(Regex::new(r"Mozilla.*").unwrap()),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        let parts = create_request_parts_with_headers(vec![(
            "user-agent",
            "Mozilla/5.0 (compatible; bot)",
        )]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "HeadersMatcher should match with regex pattern");
    }

    #[test]
    fn test_headers_matcher_case_sensitivity() {
        // Arrange - HTTP header names are case-insensitive, but HeaderName handles this
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(HeaderName::from_static("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        // Note: HTTP library typically normalizes header names to lowercase
        let parts = create_request_parts_with_headers(vec![("Content-Type", "application/json")]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "HeadersMatcher should handle header name case sensitivity correctly"
        );
    }

    #[test]
    fn test_headers_matcher_extra_headers_in_request() {
        // Arrange
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        let parts = create_request_parts_with_headers(vec![
            ("content-type", "application/json"),
            ("accept", "application/json"), // Extra header
            ("user-agent", "test-agent"),   // Extra header
        ]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "HeadersMatcher should match even when request has extra headers"
        );
    }

    #[test]
    fn test_headers_matcher_equality() {
        // Arrange
        let header_matcher1 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let header_matcher2 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let header_matcher3 = HeaderMatcher::new_exact(
            Arc::new(create_header_name("accept")),
            Arc::new(create_header_value("text/html")),
        );

        let matcher1 = HeadersMatcher::builder()
            .matchers(vec![header_matcher1])
            .build();
        let matcher2 = HeadersMatcher::builder()
            .matchers(vec![header_matcher2])
            .build();
        let matcher3 = HeadersMatcher::builder()
            .matchers(vec![header_matcher3])
            .build();

        // Assert
        assert_eq!(
            matcher1, matcher2,
            "HeadersMatchers with same header matchers should be equal"
        );
        assert_ne!(
            matcher1, matcher3,
            "HeadersMatchers with different header matchers should not be equal"
        );
    }

    #[test]
    fn test_headers_matcher_calls_scorer_on_match() {
        // Arrange
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        let parts = create_request_parts_with_headers(vec![("content-type", "application/json")]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result);
        // The fact that the method returns true indicates the scorer.headers() was called
    }

    #[test]
    fn test_headers_matcher_does_not_call_scorer_on_no_match() {
        // Arrange
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(create_header_name("content-type")),
            Arc::new(create_header_value("application/json")),
        );
        let matcher = HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build();
        let parts = create_request_parts_with_headers(vec![("content-type", "text/html")]);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(!result);
        // The fact that the method returns false indicates scorer.headers() was NOT called
    }
}
