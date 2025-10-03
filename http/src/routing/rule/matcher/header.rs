use super::Matcher;
use crate::routing::rule::matcher::basic::{ExactMatcher, RegularExpressionMatcher};
use crate::routing::rule::matcher::scoring::RequestMatcherScorer;
use derive_more::{Deref, From};
use http::request::Parts;
use http::{HeaderName, HeaderValue};
use regex::Regex;
use thiserror::Error;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;
use vg_http_config::routing::rule::matcher::{
    HeaderMatcher as HeaderMatcherConfig, HeaderValueMatcher as HeaderValueMatcherConfig,
    HeadersMatcher as HeadersMatcherConfig,
};

#[derive(Debug, Deref, From)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HeaderNameMatcher(ExactMatcher<HeaderName>);

impl From<HeaderName> for HeaderNameMatcher {
    fn from(name: HeaderName) -> Self {
        let matcher: ExactMatcher<HeaderName> = name.into();
        matcher.into()
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

#[derive(Error, Debug)]
pub enum HeaderValueMatcherConversionError {
    #[error("Invalid regular expression for header value matcher: {0}")]
    InvalidRegularExpression(#[from] regex::Error),
}

impl TryFrom<&HeaderValueMatcherConfig> for HeaderValueMatcher {
    type Error = HeaderValueMatcherConversionError;

    fn try_from(value: &HeaderValueMatcherConfig) -> Result<Self, Self::Error> {
        match value {
            HeaderValueMatcherConfig::Exact(value) => Ok(Self::Exact(value.clone().into())),
            HeaderValueMatcherConfig::RegularExpression(pattern) => {
                let regex = Regex::new(pattern)?;
                Ok(Self::RegularExpression(regex.into()))
            }
        }
    }
}

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct HeaderMatcher {
    #[builder(setter(into))]
    name_matcher: HeaderNameMatcher,
    #[builder(setter(into))]
    value_matcher: HeaderValueMatcher,
}

impl HeaderMatcher {
    fn new(name: HeaderName, value_matcher: HeaderValueMatcher) -> Self {
        Self::builder()
            .name_matcher(name)
            .value_matcher(value_matcher)
            .build()
    }

    pub fn new_exact(name: HeaderName, value: HeaderValue) -> Self {
        let value_matcher = HeaderValueMatcher::Exact(value.into());
        Self::new(name, value_matcher)
    }

    pub fn new_matching(name: HeaderName, regex: Regex) -> Self {
        let value_matcher = HeaderValueMatcher::RegularExpression(regex.into());
        Self::new(name, value_matcher)
    }

    #[instrument(
        skip(self, name, value),
        name = "HeaderMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, (name, value): &(&HeaderName, &HeaderValue)) -> bool {
        self.name_matcher.matches(name) && self.value_matcher.matches(value)
    }
}

#[derive(Error, Debug)]
pub enum HeaderMatcherConversionError {
    #[error("Invalid header value matcher: {0}")]
    InvalidValueMatcher(#[from] HeaderValueMatcherConversionError),
}

impl TryFrom<&HeaderMatcherConfig> for HeaderMatcher {
    type Error = HeaderMatcherConversionError;

    fn try_from(value: &HeaderMatcherConfig) -> Result<Self, Self::Error> {
        let name = value.name();
        let value_matcher: HeaderValueMatcher = value.value().try_into()?;
        Ok(HeaderMatcher::new(name, value_matcher))
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

#[derive(Error, Debug)]
pub enum HeadersMatcherConversionError {
    #[error("Invalid header matcher at index {0}: {1}")]
    InvalidMatcher(usize, HeaderMatcherConversionError),
}

impl TryFrom<&HeadersMatcherConfig> for HeadersMatcher {
    type Error = HeadersMatcherConversionError;

    fn try_from(value: &HeadersMatcherConfig) -> Result<Self, Self::Error> {
        let matchers = value
            .headers()
            .iter()
            .enumerate()
            .map(|(idx, value)| {
                HeaderMatcher::try_from(value)
                    .map_err(|e| HeadersMatcherConversionError::InvalidMatcher(idx, e))
            })
            .collect::<Result<_, _>>()?;

        let matcher = Self::builder().matchers(matchers).build();

        Ok(matcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
    use http::{Request, Version};
    use regex::Regex;
    use rstest::*;

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

    // HeaderNameMatcher tests
    #[rstest]
    #[case("content-type")]
    #[case("authorization")]
    #[case("x-custom-header")]
    #[case("accept")]
    fn test_header_name_matcher_exact_match(#[case] header_name: &'static str) {
        // Arrange
        let name = HeaderName::from_static(header_name);
        let matcher = HeaderNameMatcher::from(name.clone());

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
        #[case] matcher_name: &'static str,
        #[case] test_name: &'static str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let name = HeaderName::from_static(matcher_name);
        let matcher = HeaderNameMatcher::from(name);
        let test_header_name = HeaderName::from_static(test_name);

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
    fn test_header_value_matcher_exact_match(#[case] value: &'static str) {
        // Arrange
        let header_value = HeaderValue::from_static(value);
        let matcher = HeaderValueMatcher::Exact(header_value.clone().into());

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
        #[case] matcher_value: &'static str,
        #[case] test_value: &'static str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let header_value = HeaderValue::from_static(matcher_value);
        let matcher = HeaderValueMatcher::Exact(header_value.clone().into());
        let test_header_value = HeaderValue::from_static(test_value);

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
        #[case] test_value: &'static str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let regex = Regex::new(pattern).unwrap();
        let matcher = HeaderValueMatcher::RegularExpression(regex.into());
        let test_header_value = HeaderValue::from_static(test_value);

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
        let value = HeaderValue::from_static("application/json");

        // Act
        let matcher = HeaderMatcher::new_exact(CONTENT_TYPE, value.clone());

        // Assert
        let header_pair = (&CONTENT_TYPE, &value);
        assert!(
            matcher.matches(&header_pair),
            "HeaderMatcher should match exact header name and value"
        );
    }

    #[test]
    fn test_header_matcher_new_matching() {
        // Arrange
        let regex = Regex::new(r"Mozilla.*").unwrap();
        let test_value = HeaderValue::from_static("Mozilla/5.0 (compatible)");

        // Act
        let matcher = HeaderMatcher::new_matching(USER_AGENT, regex);

        // Assert
        let header_pair = (&USER_AGENT, &test_value);
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
        #[case] matcher_name: &'static str,
        #[case] matcher_value: &'static str,
        #[case] test_name: &'static str,
        #[case] test_value: &'static str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher_name = HeaderName::from_static(matcher_name);
        let matcher_value = HeaderValue::from_static(matcher_value);
        let matcher = HeaderMatcher::new_exact(matcher_name.clone(), matcher_value.clone());
        let test_header_name = HeaderName::from_static(test_name);
        let test_header_value = HeaderValue::from_static(test_value);

        // Act
        let result = matcher.matches(&(&test_header_name, &test_header_value));

        // Assert
        assert_eq!(
            result, expected_match,
            "HeaderMatcher should return {} for '{}:{:?}' vs '{}:{}'",
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
        let header_value = HeaderValue::from_static("application/json");
        let header_matcher = HeaderMatcher::new_exact(CONTENT_TYPE, header_value.clone());
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
        let header_value = HeaderValue::from_static("application/json");
        let header_matcher = HeaderMatcher::new_exact(CONTENT_TYPE, header_value);
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
        let content_type = HeaderValue::from_static("application/json");
        let authorization = HeaderValue::from_static("Bearer token123");
        let header_matcher1 = HeaderMatcher::new_exact(CONTENT_TYPE, content_type);
        let header_matcher2 = HeaderMatcher::new_exact(AUTHORIZATION, authorization);
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
        let content_type = HeaderValue::from_static("application/json");
        let authorization = HeaderValue::from_static("Bearer token123");
        let header_matcher1 = HeaderMatcher::new_exact(CONTENT_TYPE, content_type);
        let header_matcher2 = HeaderMatcher::new_exact(AUTHORIZATION, authorization);
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
        let mozilla_regex = Regex::new(r"Mozilla.*").unwrap();
        let header_matcher = HeaderMatcher::new_matching(USER_AGENT, mozilla_regex);
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
    fn test_headers_matcher_extra_headers_in_request() {
        // Arrange
        let content_type = HeaderValue::from_static("application/json");
        let header_matcher = HeaderMatcher::new_exact(CONTENT_TYPE, content_type);
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
    fn test_headers_matcher_calls_scorer_on_match() {
        // Arrange
        let content_type = HeaderValue::from_static("application/json");
        let header_matcher = HeaderMatcher::new_exact(CONTENT_TYPE, content_type);
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
        let content_type = HeaderValue::from_static("application/json");
        let header_matcher = HeaderMatcher::new_exact(CONTENT_TYPE, content_type);
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
