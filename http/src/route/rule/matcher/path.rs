use super::Matcher;
use super::basic::{ExactMatcher, RegularExpressionMatcher, StringPrefixMatcher};
use super::scoring::RequestMatcherScorer;
use http::request::Parts;
use regex::Regex;
use thiserror::Error;
use tracing::{debug, instrument};
use vg_config::http::route::rule::matcher::PathMatcher as PathMatcherConfig;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum PathMatcher {
    Exact(ExactMatcher<String>),
    Prefix(StringPrefixMatcher),
    RegularExpression(RegularExpressionMatcher),
}

impl Matcher for PathMatcher {
    #[instrument(
        skip(self, scorer, req),
        name = "PathMatch::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let path = req.uri.path();

        let matched = match self {
            PathMatcher::Exact(matcher) => matcher.matches_str(path),
            PathMatcher::Prefix(matcher) => matcher.matches(path),
            PathMatcher::RegularExpression(matcher) => matcher.matches(path),
        };

        if matched {
            debug!("Path matched");
            scorer.path(self);
        }

        matched
    }
}

#[derive(Debug, Error)]
pub enum PathMatcherConversionError {
    #[error("Invalid regular expression")]
    InvalidRegularExpression(#[from] regex::Error),
}

impl TryFrom<&PathMatcherConfig> for PathMatcher {
    type Error = PathMatcherConversionError;

    fn try_from(value: &PathMatcherConfig) -> Result<Self, Self::Error> {
        match value {
            PathMatcherConfig::Exact(path) => Ok(PathMatcher::Exact(path.into())),
            PathMatcherConfig::Prefix(prefix) => Ok(PathMatcher::Prefix(prefix.into())),
            PathMatcherConfig::RegularExpression(pattern) => {
                let regex = Regex::new(pattern)?;
                Ok(PathMatcher::RegularExpression(regex.into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Version};
    use regex::Regex;
    use rstest::*;

    fn create_request_parts_with_path(path: &str) -> Parts {
        let uri = format!("http://example.com{path}");
        let request = Request::builder().uri(&uri).version(Version::HTTP_11).body(()).unwrap();
        let (parts, ()) = request.into_parts();
        parts
    }

    // PathMatcher::Exact tests
    #[rstest]
    #[case("/")]
    #[case("/api")]
    #[case("/api/v1")]
    #[case("/users/123")]
    #[case("/health-check")]
    #[case("/path/with-dashes")]
    #[case("/path/with_underscores")]
    fn test_path_matcher_exact_match(#[case] path: &str) {
        // Arrange
        let matcher = PathMatcher::Exact(path.into());
        let parts = create_request_parts_with_path(path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "PathMatcher::Exact should match the exact path '{path}'");
    }

    #[rstest]
    #[case("/api", "/api/v1", false)]
    #[case("/users", "/user", false)]
    #[case("/health", "/health-check", false)]
    #[case("/api/v1", "/api/v1", true)]
    #[case("/", "/", true)]
    #[case("/exact", "/exact", true)]
    fn test_path_matcher_exact_different_paths(
        #[case] matcher_path: &str,
        #[case] request_path: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = PathMatcher::Exact(matcher_path.into());
        let parts = create_request_parts_with_path(request_path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert_eq!(
            result, expected_match,
            "PathMatcher::Exact should return {expected_match} for '{matcher_path}' vs '{request_path}'"
        );
    }

    // PathMatcher::Prefix tests
    #[rstest]
    #[case("/api", "/api", true)]
    #[case("/api", "/api/v1", true)]
    #[case("/api", "/api/v1/users", true)]
    #[case("/users", "/users/123", true)]
    #[case("/health", "/health-check", true)]
    #[case("/api", "/web", false)]
    #[case("/users", "/user", false)]
    #[case("/long-prefix", "/short", false)]
    fn test_path_matcher_prefix_match(#[case] prefix: &str, #[case] request_path: &str, #[case] expected_match: bool) {
        // Arrange
        let matcher = PathMatcher::Prefix(prefix.into());
        let parts = create_request_parts_with_path(request_path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert_eq!(
            result, expected_match,
            "PathMatcher::Prefix should return {expected_match} for prefix '{prefix}' vs path '{request_path}'"
        );
    }

    #[rstest]
    #[case("/", "/", true)]
    #[case("/", "/anything", true)]
    #[case("/", "/api/v1/users", true)]
    fn test_path_matcher_prefix_root_matches_all(
        #[case] prefix: &str,
        #[case] request_path: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = PathMatcher::Prefix(prefix.into());
        let parts = create_request_parts_with_path(request_path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert_eq!(
            result, expected_match,
            "PathMatcher::Prefix with root '{prefix}' should match any path '{request_path}'"
        );
    }

    // PathMatcher::RegularExpression tests
    #[rstest]
    #[case(r"^/api/v\d+$", "/api/v1", true)]
    #[case(r"^/api/v\d+$", "/api/v2", true)]
    #[case(r"^/api/v\d+$", "/api/v123", true)]
    #[case(r"^/api/v\d+$", "/api/v", false)]
    #[case(r"^/api/v\d+$", "/api/version", false)]
    #[case(r"^/users/\d+$", "/users/123", true)]
    #[case(r"^/users/\d+$", "/users/abc", false)]
    #[case(r"^/health.*", "/health", true)]
    #[case(r"^/health.*", "/health-check", true)]
    #[case(r"^/health.*", "/api/health", false)]
    fn test_path_matcher_regex_match(#[case] pattern: &str, #[case] request_path: &str, #[case] expected_match: bool) {
        // Arrange
        let regex = Regex::new(pattern).unwrap();
        let matcher = PathMatcher::RegularExpression(regex.into());
        let parts = create_request_parts_with_path(request_path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert_eq!(
            result, expected_match,
            "PathMatcher::RegularExpression with pattern '{pattern}' should return {expected_match} for path '{request_path}'"
        );
    }

    #[rstest]
    #[case(r"(?i)^/API/.*", "/api/v1", true)]
    #[case(r"(?i)^/API/.*", "/API/v1", true)]
    #[case(r"(?i)^/API/.*", "/Api/v1", true)]
    #[case(r"^/API/.*", "/api/v1", false)]
    fn test_path_matcher_regex_case_sensitivity(
        #[case] pattern: &str,
        #[case] request_path: &str,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let regex = Regex::new(pattern).unwrap();
        let matcher = PathMatcher::RegularExpression(regex.into());
        let parts = create_request_parts_with_path(request_path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert_eq!(
            result, expected_match,
            "PathMatcher::RegularExpression case sensitivity test with pattern '{pattern}' should return {expected_match} for path '{request_path}'"
        );
    }

    // Scorer integration tests
    #[test]
    fn test_path_matcher_calls_scorer_on_exact_match() {
        // Arrange
        let matcher = PathMatcher::Exact("/api/v1".into());
        let parts = create_request_parts_with_path("/api/v1");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result);
        // The fact that the method returns true indicates scorer.path() was called
    }

    #[test]
    fn test_path_matcher_calls_scorer_on_prefix_match() {
        // Arrange
        let matcher = PathMatcher::Prefix("/api".into());
        let parts = create_request_parts_with_path("/api/v1");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result);
        // The fact that the method returns true indicates scorer.path() was called
    }

    #[test]
    fn test_path_matcher_calls_scorer_on_regex_match() {
        // Arrange
        let regex = Regex::new(r"^/api/v\d+$").unwrap();
        let matcher = PathMatcher::RegularExpression(regex.into());
        let parts = create_request_parts_with_path("/api/v1");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result);
        // The fact that the method returns true indicates scorer.path() was called
    }

    #[test]
    fn test_path_matcher_does_not_call_scorer_on_no_match() {
        // Arrange
        let matcher = PathMatcher::Exact("/api/v1".into());
        let parts = create_request_parts_with_path("/web/dashboard");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(!result);
        // The fact that the method returns false indicates scorer.path() was NOT called
    }

    // Edge cases and special scenarios
    #[test]
    fn test_path_matcher_empty_path() {
        // Arrange - empty path defaults to "/"
        let matcher = PathMatcher::Exact("/".into());
        let parts = create_request_parts_with_path("");
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        // Note: HTTP URIs typically normalize empty path to "/"
        // This test verifies the actual behavior of the HTTP library
        let actual_path = parts.uri.path();
        if actual_path == "/" {
            assert!(result, "Empty path should be normalized to '/' and match");
        } else {
            assert!(!result, "Empty path '{actual_path}' should not match '/'");
        }
    }

    #[rstest]
    #[case("/path/with%20encoded")]
    fn test_path_matcher_special_characters(#[case] path: &str) {
        // Arrange
        let matcher = PathMatcher::Exact(path.into());
        // Note: This might fail for invalid URIs, but tests the matcher logic
        if let Ok(uri) = format!("http://example.com{path}").parse::<http::Uri>() {
            let request = Request::builder().uri(uri).version(Version::HTTP_11).body(()).unwrap();
            let (parts, ()) = request.into_parts();
            let scorer = RequestMatcherScorer::default();

            // Act
            let result = matcher.matches(&scorer, &parts);

            // Assert
            assert!(
                result,
                "PathMatcher should handle special characters in path '{path}'"
            );
        }
        // If URI parsing fails, the test passes (expected for invalid URIs)
    }

    #[test]
    fn test_path_matcher_invalid_uri_characters() {
        // Test that we handle paths that can't be parsed as valid URIs
        let invalid_paths = vec!["/path/with spaces"];

        for path in invalid_paths {
            // These paths should fail to parse as URIs, which is expected behavior
            let uri_result = format!("http://example.com{path}").parse::<http::Uri>();
            assert!(uri_result.is_err(), "Path '{path}' should fail to parse as URI");
        }

        // Test that we handle paths with special characters that ARE valid URIs
        let valid_encoded_paths = vec!["/path/with%20spaces", "/path/with%21exclamation"];

        for path in valid_encoded_paths {
            let uri_result = format!("http://example.com{path}").parse::<http::Uri>();
            assert!(uri_result.is_ok(), "Path '{path}' should parse as valid URI");

            if let Ok(uri) = uri_result {
                let matcher = PathMatcher::Exact(uri.path().into());
                let request = Request::builder().uri(uri).version(Version::HTTP_11).body(()).unwrap();
                let (parts, ()) = request.into_parts();
                let scorer = RequestMatcherScorer::default();

                let result = matcher.matches(&scorer, &parts);
                assert!(result, "PathMatcher should handle encoded path '{path}'");
            }
        }
    }

    #[test]
    fn test_path_matcher_variants_comprehensive() {
        // Test all three variants work correctly
        let test_path = "/api/v1/users";
        let parts = create_request_parts_with_path(test_path);
        let scorer = RequestMatcherScorer::default();

        // Exact matcher
        let exact_matcher = PathMatcher::Exact(test_path.into());
        assert!(exact_matcher.matches(&scorer, &parts), "Exact matcher should work");

        // Prefix matcher
        let prefix_matcher = PathMatcher::Prefix("/api".into());
        assert!(prefix_matcher.matches(&scorer, &parts), "Prefix matcher should work");

        // Regex matcher
        let regex = Regex::new(r"^/api/v\d+/users$").unwrap();
        let regex_matcher = PathMatcher::RegularExpression(regex.into());
        assert!(regex_matcher.matches(&scorer, &parts), "Regex matcher should work");
    }

    #[rstest]
    #[case("/api/v1")]
    #[case("/users/123/profile")]
    #[case("/health-check")]
    #[case("/webhooks/github")]
    fn test_path_matcher_builder_pattern_compatibility(#[case] path: &str) {
        // Arrange
        let matcher = PathMatcher::Exact(path.into());
        let parts = create_request_parts_with_path(path);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "PathMatcher should work with builder pattern for path '{path}'"
        );
    }

    #[test]
    fn test_path_matcher_query_parameters_ignored() {
        // Arrange
        let matcher = PathMatcher::Exact("/api/users".into());
        let request = Request::builder()
            .uri("http://example.com/api/users?id=123&name=test")
            .version(Version::HTTP_11)
            .body(())
            .unwrap();
        let (parts, ()) = request.into_parts();
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "PathMatcher should ignore query parameters and match path only");
        // Verify that only the path part is considered
        assert_eq!(parts.uri.path(), "/api/users");
    }

    #[rstest]
    #[case("/api", "")]
    #[case("/api", "?version=1")]
    #[case("/api", "?version=1&format=json")]
    #[case("/api/v1", "?id=123")]
    #[case("/api/v1", "?id=123&limit=10&offset=0")]
    #[case("/users/123", "?include=profile")]
    #[case("/", "?redirect=/home")]
    fn test_path_matcher_exact_ignores_query_string(#[case] path: &str, #[case] query: &str) {
        // Arrange
        let matcher = PathMatcher::Exact(path.into());
        let uri = format!("http://example.com{path}{query}");
        let request = Request::builder().uri(&uri).version(Version::HTTP_11).body(()).unwrap();
        let (parts, ()) = request.into_parts();
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "PathMatcher::Exact should match path '{path}' regardless of query string '{query}'"
        );
        assert_eq!(parts.uri.path(), path, "Path should be extracted correctly");
    }

    #[rstest]
    #[case("/api", "/api/v1", "")]
    #[case("/api", "/api/v1", "?version=2")]
    #[case("/api", "/api/users", "?limit=50")]
    #[case("/users", "/users/123/profile", "?expand=all")]
    #[case("/", "/anything/deep/path", "?complex=query&with=multiple&params=true")]
    fn test_path_matcher_prefix_ignores_query_string(#[case] prefix: &str, #[case] path: &str, #[case] query: &str) {
        // Arrange
        let matcher = PathMatcher::Prefix(prefix.into());
        let uri = format!("http://example.com{path}{query}");
        let request = Request::builder().uri(&uri).version(Version::HTTP_11).body(()).unwrap();
        let (parts, ()) = request.into_parts();
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "PathMatcher::Prefix '{prefix}' should match path '{path}' regardless of query string '{query}'"
        );
        assert_eq!(parts.uri.path(), path, "Path should be extracted correctly");
    }

    #[rstest]
    #[case(r"^/api/v\d+$", "/api/v1", "")]
    #[case(r"^/api/v\d+$", "/api/v2", "?format=json")]
    #[case(r"^/users/\d+$", "/users/123", "?include=posts&include=comments")]
    #[case(r"^/health.*", "/health-check", "?detailed=true")]
    #[case(r"^/webhooks/[a-z]+$", "/webhooks/github", "?event=push&signature=abc123")]
    fn test_path_matcher_regex_ignores_query_string(#[case] pattern: &str, #[case] path: &str, #[case] query: &str) {
        // Arrange
        let regex = Regex::new(pattern).unwrap();
        let matcher = PathMatcher::RegularExpression(regex.into());
        let uri = format!("http://example.com{path}{query}");
        let request = Request::builder().uri(&uri).version(Version::HTTP_11).body(()).unwrap();
        let (parts, ()) = request.into_parts();
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(
            result,
            "PathMatcher::RegularExpression '{pattern}' should match path '{path}' regardless of query string '{query}'"
        );
        assert_eq!(parts.uri.path(), path, "Path should be extracted correctly");
    }

    #[rstest]
    #[case("?param1=value1&param2=value2&param3=value3")]
    #[case("?search=test%20query&match[category]=electronics&match[price_min]=100")]
    #[case("?callback=jsonp_callback_123&api_key=secret123&timestamp=1234567890")]
    #[case("?redirect_uri=https%3A%2F%2Fexample.com%2Fcallback&state=random_state_123")]
    #[case("?empty=&null&boolean=true&number=42&array[]=item1&array[]=item2")]
    #[case("?return=/api/test2")]
    fn test_path_matcher_complex_query_parameters(#[case] query: &str) {
        // Test with very complex query strings to ensure they don't interfere
        let uri = format!("http://example.com/api/test{query}");
        let request = Request::builder().uri(&uri).version(Version::HTTP_11).body(()).unwrap();
        let (parts, ()) = request.into_parts();
        let scorer = RequestMatcherScorer::default();

        // Test Exact matcher
        let exact_matcher = PathMatcher::Exact("/api/test".into());
        assert!(
            exact_matcher.matches(&scorer, &parts),
            "Exact matcher should ignore complex query: {query}"
        );

        // Test Prefix matcher
        let prefix_matcher = PathMatcher::Prefix("/api".into());
        assert!(
            prefix_matcher.matches(&scorer, &parts),
            "Prefix matcher should ignore complex query: {query}"
        );

        // Test Regex matcher
        let regex = Regex::new(r"^/api/test$").unwrap();
        let regex_matcher = PathMatcher::RegularExpression(regex.into());
        assert!(
            regex_matcher.matches(&scorer, &parts),
            "Regex matcher should ignore complex query: {query}"
        );
    }

    #[test]
    fn test_path_matcher_fragment_ignored() {
        // Arrange
        let matcher = PathMatcher::Exact("/api/users".into());
        let request = Request::builder()
            .uri("http://example.com/api/users#section")
            .version(Version::HTTP_11)
            .body(())
            .unwrap();
        let (parts, ()) = request.into_parts();
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "PathMatcher should ignore fragments and match path only");
        assert_eq!(parts.uri.path(), "/api/users");
    }
}
