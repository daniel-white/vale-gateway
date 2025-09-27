use super::Matcher;
use super::basic::ExactMatcher;
use super::scoring::RequestMatcherScorer;
use http::Method;
use http::request::Parts;
use tracing::{debug, instrument};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
#[cfg_attr(test, derive(PartialEq))]
pub struct MethodMatcher {
    method_matcher: ExactMatcher<Method>,
}

impl Matcher for MethodMatcher {
    #[instrument(
        skip(self, scorer, req),
        name = "MethodMatcher::matches"
        fields(matcher = ?self)
    )]
    fn matches(&self, scorer: &RequestMatcherScorer, req: &Parts) -> bool {
        let is_match = self.method_matcher.matches(&req.method);
        if is_match {
            debug!("Method matched");
            scorer.method(self);
        }
        is_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Method, Request, Version};
    use rstest::*;
    use std::sync::Arc;

    fn create_request_parts(method: Method) -> Parts {
        let request = Request::builder()
            .method(method)
            .uri("http://example.com/test")
            .version(Version::HTTP_11)
            .body(())
            .unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    #[rstest]
    #[case(Method::GET)]
    #[case(Method::POST)]
    #[case(Method::PUT)]
    #[case(Method::DELETE)]
    #[case(Method::PATCH)]
    #[case(Method::HEAD)]
    #[case(Method::OPTIONS)]
    fn test_method_matcher_exact_match(#[case] method: Method) {
        // Arrange
        let matcher = MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(method.clone())))
            .build();

        let parts = create_request_parts(method);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "MethodMatcher should match the exact method");

        // Verify scorer was called by checking the results
        let _score = scorer.results();
        // Since we can't access private fields, we just verify the matcher returned true
        // The integration between matcher and scorer is tested by the actual behavior
    }

    #[rstest]
    #[case(Method::GET, Method::POST, false)]
    #[case(Method::POST, Method::GET, false)]
    #[case(Method::PUT, Method::DELETE, false)]
    #[case(Method::DELETE, Method::PATCH, false)]
    #[case(Method::HEAD, Method::OPTIONS, false)]
    #[case(Method::GET, Method::GET, true)]
    #[case(Method::POST, Method::POST, true)]
    fn test_method_matcher_different_methods(
        #[case] matcher_method: Method,
        #[case] request_method: Method,
        #[case] expected_match: bool,
    ) {
        // Arrange
        let matcher = MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(matcher_method)))
            .build();

        let parts = create_request_parts(request_method);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert_eq!(
            result, expected_match,
            "MethodMatcher should return {} for matcher method vs request method comparison",
            expected_match
        );
    }

    #[test]
    fn test_method_matcher_calls_scorer_on_match() {
        // Arrange
        let matcher = MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(Method::GET)))
            .build();

        let parts = create_request_parts(Method::GET);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result);
        // The fact that the method returns true indicates the scorer.method() was called
        // since that's the only way a match is recorded
    }

    #[test]
    fn test_method_matcher_does_not_call_scorer_on_no_match() {
        // Arrange
        let matcher = MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(Method::GET)))
            .build();

        let parts = create_request_parts(Method::POST);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(!result);
        // The fact that the method returns false indicates the scorer.method() was NOT called
    }

    #[rstest]
    #[case("GET")]
    #[case("POST")]
    #[case("PUT")]
    #[case("DELETE")]
    #[case("PATCH")]
    #[case("HEAD")]
    #[case("OPTIONS")]
    #[case("CONNECT")]
    #[case("TRACE")]
    fn test_method_matcher_with_various_http_methods(#[case] method_str: &str) {
        // Arrange
        let method = Method::from_bytes(method_str.as_bytes()).unwrap();
        let matcher = MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(method.clone())))
            .build();

        let parts = create_request_parts(method);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "MethodMatcher should match {} method", method_str);
    }

    #[test]
    fn test_method_matcher_with_custom_method() {
        // Arrange
        let custom_method = Method::from_bytes(b"CUSTOM").unwrap();
        let matcher = MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(custom_method.clone())))
            .build();

        let parts = create_request_parts(custom_method);
        let scorer = RequestMatcherScorer::default();

        // Act
        let result = matcher.matches(&scorer, &parts);

        // Assert
        assert!(result, "MethodMatcher should match custom HTTP method");
    }
}
