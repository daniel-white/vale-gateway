use crate::request::matchers::RequestMatchDetails;
use crate::request::matchers::header::HeadersMatcher;
use crate::request::matchers::method::MethodMatcher;
use crate::request::matchers::path::PathMatcher;
use crate::request::matchers::query_param::QueryParamsMatcher;
use std::cell::Cell;
use std::cmp::Ordering;
use std::sync::Arc;
use tracing::instrument;
use typed_builder::TypedBuilder;

#[derive(PartialEq, Eq, Debug, TypedBuilder)]
pub struct RequestMatchScore {
    path_exact: bool,
    path_weight: Option<usize>,
    path_prefix: Option<Arc<String>>, // Not for scoring, just for info
    method: bool,
    headers_weight: Option<usize>,
    query_params_weight: Option<usize>,
}

impl RequestMatchDetails for RequestMatchScore {
    fn path_prefix(&self) -> Option<Arc<String>> {
        self.path_prefix.clone()
    }
}

impl PartialOrd for RequestMatchScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RequestMatchScore {
    #[instrument(skip(self, other), name = "Score::cmp")]
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            return Ordering::Equal;
        }

        match (self.path_exact, other.path_exact) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        };

        match (self.path_weight, other.path_weight) {
            (Some(len1), Some(len2)) => match len1.cmp(&len2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        };

        match (self.method, other.method) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        };

        match (self.headers_weight, other.headers_weight) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        };

        match (self.query_params_weight, other.query_params_weight) {
            (Some(count1), Some(count2)) => match count1.cmp(&count2) {
                Ordering::Less => return Ordering::Greater,
                Ordering::Greater => return Ordering::Less,
                Ordering::Equal => {}
            },
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        };

        Ordering::Equal
    }
}

#[derive(Default)]
pub struct RequestMatcherScorer {
    path_exact: Cell<bool>,
    path_weight: Cell<Option<usize>>,
    path_prefix: Cell<Option<Arc<String>>>,
    method: Cell<bool>,
    headers_weight: Cell<Option<usize>>,
    query_params_weight: Cell<Option<usize>>,
}

impl RequestMatcherScorer {
    pub fn results(self) -> RequestMatchScore {
        RequestMatchScore::builder()
            .method(self.method.into_inner())
            .headers_weight(self.headers_weight.into_inner())
            .path_exact(self.path_exact.into_inner())
            .path_prefix(self.path_prefix.into_inner())
            .path_weight(self.path_weight.into_inner())
            .query_params_weight(self.query_params_weight.into_inner())
            .build()
    }

    pub fn path(&self, path_match: &PathMatcher) {
        match path_match {
            PathMatcher::Exact(_) => {
                self.path_exact.replace(true);
            }
            PathMatcher::Prefix(matcher) => {
                let prefix = matcher.prefix();
                let weight = matcher.weight();
                self.path_prefix.replace(Some(prefix));
                self.path_weight.replace(Some(weight));
            }
            PathMatcher::RegularExpression(matcher) => {
                let weight = matcher.weight();
                self.path_weight.replace(Some(weight));
            }
        };
    }

    pub fn method(&self, _matcher: &MethodMatcher) {
        self.method.replace(true);
    }

    pub fn headers(&self, matcher: &HeadersMatcher) {
        let weight = matcher.weight();
        self.headers_weight.replace(Some(weight));
    }

    pub fn query_params(&self, matcher: &QueryParamsMatcher) {
        let weight = matcher.weight();
        self.query_params_weight.replace(Some(weight));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::matchers::basic::{
        ExactMatcher, RegularExpressionMatcher, StringPrefixMatcher,
    };
    use crate::request::matchers::header::{HeaderMatcher, HeadersMatcher};
    use crate::request::matchers::method::MethodMatcher;
    use crate::request::matchers::path::PathMatcher;
    use crate::request::matchers::query_param::{
        QueryParamMatcher, QueryParamNameMatcher, QueryParamValueMatcher, QueryParamsMatcher,
    };
    use assertables::*;
    use http::{HeaderName, HeaderValue, Method};
    use regex::Regex;
    use rstest::*;
    use std::sync::Arc;

    #[fixture]
    fn path_exact_matcher() -> PathMatcher {
        PathMatcher::Exact(ExactMatcher::new(Arc::new("/api/v1/test".to_string())))
    }

    #[fixture]
    fn path_prefix_matcher() -> PathMatcher {
        PathMatcher::Prefix(StringPrefixMatcher::new(Arc::new("/api".to_string())))
    }

    #[fixture]
    fn path_regex_matcher() -> PathMatcher {
        let regex = Regex::new(r"^/api/v[0-9]+/.*$").unwrap();
        PathMatcher::RegularExpression(RegularExpressionMatcher::new(Arc::new(regex)))
    }

    #[fixture]
    fn method_matcher() -> MethodMatcher {
        MethodMatcher::builder()
            .method_matcher(ExactMatcher::new(Arc::new(Method::GET)))
            .build()
    }

    #[fixture]
    fn headers_matcher_single() -> HeadersMatcher {
        let header_matcher = HeaderMatcher::new_exact(
            Arc::new(HeaderName::from_static("content-type")),
            Arc::new(HeaderValue::from_static("application/json")),
        );
        HeadersMatcher::builder()
            .matchers(vec![header_matcher])
            .build()
    }

    #[fixture]
    fn headers_matcher_multiple() -> HeadersMatcher {
        let header1 = HeaderMatcher::new_exact(
            Arc::new(HeaderName::from_static("content-type")),
            Arc::new(HeaderValue::from_static("application/json")),
        );
        let header2 = HeaderMatcher::new_exact(
            Arc::new(HeaderName::from_static("accept")),
            Arc::new(HeaderValue::from_static("application/json")),
        );
        HeadersMatcher::builder()
            .matchers(vec![header1, header2])
            .build()
    }

    #[fixture]
    fn query_params_matcher_single() -> QueryParamsMatcher {
        let param_matcher = QueryParamMatcher::builder()
            .name_matcher(
                QueryParamNameMatcher::builder()
                    .matcher(ExactMatcher::new(Arc::new("version".to_string())))
                    .build(),
            )
            .value_matcher(QueryParamValueMatcher::Exact(ExactMatcher::new(Arc::new(
                "v1".to_string(),
            ))))
            .build();
        QueryParamsMatcher::builder()
            .matchers(vec![param_matcher])
            .build()
    }

    #[fixture]
    fn query_params_matcher_multiple() -> QueryParamsMatcher {
        let param1 = QueryParamMatcher::builder()
            .name_matcher(
                QueryParamNameMatcher::builder()
                    .matcher(ExactMatcher::new(Arc::new("version".to_string())))
                    .build(),
            )
            .value_matcher(QueryParamValueMatcher::Exact(ExactMatcher::new(Arc::new(
                "v1".to_string(),
            ))))
            .build();
        let param2 = QueryParamMatcher::builder()
            .name_matcher(
                QueryParamNameMatcher::builder()
                    .matcher(ExactMatcher::new(Arc::new("format".to_string())))
                    .build(),
            )
            .value_matcher(QueryParamValueMatcher::Exact(ExactMatcher::new(Arc::new(
                "json".to_string(),
            ))))
            .build();
        QueryParamsMatcher::builder()
            .matchers(vec![param1, param2])
            .build()
    }

    #[rstest]
    fn test_request_match_score_builder() {
        let score = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(Some(10))
            .path_prefix(Some(Arc::new("/api".to_string())))
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        assert!(score.path_exact);
        assert_eq!(score.path_weight, Some(10));
        assert_eq!(score.path_prefix.as_ref().unwrap().as_str(), "/api");
        assert!(score.method);
        assert_eq!(score.headers_weight, Some(2));
        assert_eq!(score.query_params_weight, Some(1));
    }

    #[rstest]
    fn test_request_match_details_implementation() {
        let score = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(None)
            .path_prefix(Some(Arc::new("/test/path".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let prefix = score.path_prefix();
        assert!(prefix.is_some());
        assert_eq!(prefix.unwrap().as_str(), "/test/path");
    }

    #[rstest]
    fn test_request_match_scorer_results() {
        let scorer = RequestMatcherScorer::default();

        let result = scorer.results();
        assert!(!result.path_exact);
        assert_eq!(result.path_weight, None);
        assert_eq!(result.path_prefix, None);
        assert!(!result.method);
        assert_eq!(result.headers_weight, None);
        assert_eq!(result.query_params_weight, None);
    }

    #[rstest]
    fn test_scorer_path_exact(path_exact_matcher: PathMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.path(&path_exact_matcher);

        let result = scorer.results();
        assert!(result.path_exact);
        assert_eq!(result.path_weight, None);
        assert_eq!(result.path_prefix, None);
    }

    #[rstest]
    fn test_scorer_path_prefix(path_prefix_matcher: PathMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.path(&path_prefix_matcher);

        let result = scorer.results();
        assert!(!result.path_exact);
        assert_eq!(result.path_weight, Some(4)); // "/api" has length 4
        assert!(result.path_prefix.is_some());
        assert_eq!(result.path_prefix.unwrap().as_str(), "/api");
    }

    #[rstest]
    fn test_scorer_path_regex(path_regex_matcher: PathMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.path(&path_regex_matcher);

        let result = scorer.results();
        assert!(!result.path_exact);
        // Regex weight is pattern length * 4
        assert!(result.path_weight.is_some());
        assert_eq!(result.path_prefix, None);
    }

    #[rstest]
    fn test_scorer_method(method_matcher: MethodMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.method(&method_matcher);

        let result = scorer.results();
        assert!(result.method);
    }

    #[rstest]
    fn test_scorer_headers_single(headers_matcher_single: HeadersMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.headers(&headers_matcher_single);

        let result = scorer.results();
        assert_eq!(result.headers_weight, Some(1));
    }

    #[rstest]
    fn test_scorer_headers_multiple(headers_matcher_multiple: HeadersMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.headers(&headers_matcher_multiple);

        let result = scorer.results();
        assert_eq!(result.headers_weight, Some(2));
    }

    #[rstest]
    fn test_scorer_query_params_single(query_params_matcher_single: QueryParamsMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.query_params(&query_params_matcher_single);

        let result = scorer.results();
        assert_eq!(result.query_params_weight, Some(1));
    }

    #[rstest]
    fn test_scorer_query_params_multiple(query_params_matcher_multiple: QueryParamsMatcher) {
        let scorer = RequestMatcherScorer::default();
        scorer.query_params(&query_params_matcher_multiple);

        let result = scorer.results();
        assert_eq!(result.query_params_weight, Some(2));
    }

    // Tests for comparison ordering according to Gateway API specification
    // Rule 1: Exact path matches have higher precedence than prefix or regex
    #[rstest]
    fn test_path_exact_beats_prefix() {
        let exact_score = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let prefix_score = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(10))
            .path_prefix(Some(Arc::new("/api/very/long/path".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_lt!(exact_score, prefix_score);
        assert_gt!(prefix_score, exact_score);
    }

    #[rstest]
    fn test_path_exact_beats_regex() {
        let exact_score = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let regex_score = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(100)) // Very high weight for regex
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_lt!(exact_score, regex_score);
        assert_gt!(regex_score, exact_score);
    }

    // Rule 2: Among non-exact path matches, higher path weight (longer prefix/more complex regex) wins
    #[rstest]
    fn test_longer_prefix_wins() {
        let shorter_prefix = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(4)) // "/api"
            .path_prefix(Some(Arc::new("/api".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let longer_prefix = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(8)) // "/api/v1"
            .path_prefix(Some(Arc::new("/api/v1".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_lt!(longer_prefix, shorter_prefix);
        assert_gt!(shorter_prefix, longer_prefix);
    }

    #[rstest]
    fn test_more_complex_regex_wins() {
        let simple_regex = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(20)) // Simple regex weight
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let complex_regex = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(40)) // More complex regex weight
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_lt!(complex_regex, simple_regex);
        assert_gt!(simple_regex, complex_regex);
    }

    // Rule 3: Method matcher presence affects precedence
    #[rstest]
    fn test_method_matcher_beats_no_method() {
        let with_method = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let without_method = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_lt!(with_method, without_method);
        assert_gt!(without_method, with_method);
    }

    // Rule 4: Higher header count wins
    #[rstest]
    fn test_more_headers_wins() {
        let fewer_headers = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(1))
            .query_params_weight(None)
            .build();

        let more_headers = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(3))
            .query_params_weight(None)
            .build();

        assert_lt!(more_headers, fewer_headers);
        assert_gt!(fewer_headers, more_headers);
    }

    #[rstest]
    fn test_headers_present_beats_no_headers() {
        let with_headers = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(1))
            .query_params_weight(None)
            .build();

        let no_headers = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        // Headers present should be better (less than) no headers
        assert_lt!(with_headers, no_headers);
        assert_gt!(no_headers, with_headers);
    }

    // Rule 5: Higher query parameter count wins
    #[rstest]
    fn test_more_query_params_wins() {
        let fewer_params = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        let more_params = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(3))
            .build();

        assert_lt!(more_params, fewer_params);
        assert_gt!(fewer_params, more_params);
    }

    #[rstest]
    fn test_query_params_present_beats_no_query_params() {
        let with_params = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(1))
            .query_params_weight(Some(1))
            .build();

        let no_params = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(true)
            .headers_weight(Some(1))
            .query_params_weight(None)
            .build();

        // Query params present should be better (less than) no query params
        assert_lt!(with_params, no_params);
        assert_gt!(no_params, with_params);
    }

    // Complex scenarios testing multiple rules together
    #[rstest]
    fn test_complex_comparison_scenario_1() {
        // Exact path with method vs prefix path with method and headers
        let exact_with_method = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(true)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let prefix_with_method_headers = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(10))
            .path_prefix(Some(Arc::new("/api/v1/test".to_string())))
            .method(true)
            .headers_weight(Some(5))
            .query_params_weight(Some(2))
            .build();

        // Exact path should win despite fewer matchers
        assert_lt!(exact_with_method, prefix_with_method_headers);
    }

    #[rstest]
    fn test_complex_comparison_scenario_2() {
        // Same path type but different specificity levels
        let general_match = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(4)) // "/api"
            .path_prefix(Some(Arc::new("/api".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let specific_match = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(12)) // "/api/v1/users"
            .path_prefix(Some(Arc::new("/api/v1/users".to_string())))
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        // More specific match should win
        assert_lt!(specific_match, general_match);
    }

    #[rstest]
    fn test_equal_scores() {
        let score1 = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        let score2 = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(true)
            .headers_weight(Some(2))
            .query_params_weight(Some(1))
            .build();

        assert_eq!(score1, score2);
        assert_eq!(score1.cmp(&score2), Ordering::Equal);
    }

    #[rstest]
    fn test_partial_ord_consistency() {
        let score1 = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let score2 = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(5))
            .path_prefix(Some(Arc::new("/test".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let ord_result = score1.cmp(&score2);
        let partial_ord_result = score1.partial_cmp(&score2);

        assert_eq!(partial_ord_result, Some(ord_result));
    }

    // Edge cases and boundary conditions
    #[rstest]
    fn test_zero_weight_path() {
        let zero_weight = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(0))
            .path_prefix(Some(Arc::new("".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let positive_weight = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(1))
            .path_prefix(Some(Arc::new("/".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_gt!(zero_weight, positive_weight);
    }

    #[rstest]
    fn test_all_matchers_present() {
        let complete_score = RequestMatchScore::builder()
            .path_exact(true)
            .path_weight(None)
            .path_prefix(None)
            .method(true)
            .headers_weight(Some(5))
            .query_params_weight(Some(3))
            .build();

        let minimal_score = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(None)
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        assert_lt!(complete_score, minimal_score);
    }

    // Test sorting behavior with multiple scores
    #[rstest]
    fn test_sorting_multiple_scores() {
        let mut scores = [
            RequestMatchScore::builder()
                .path_exact(false)
                .path_weight(Some(5))
                .path_prefix(Some(Arc::new("/test".to_string())))
                .method(false)
                .headers_weight(None)
                .query_params_weight(None)
                .build(),
            RequestMatchScore::builder()
                .path_exact(true)
                .path_weight(None)
                .path_prefix(None)
                .method(false)
                .headers_weight(None)
                .query_params_weight(None)
                .build(),
            RequestMatchScore::builder()
                .path_exact(false)
                .path_weight(Some(10))
                .path_prefix(Some(Arc::new("/api/v1/test".to_string())))
                .method(true)
                .headers_weight(Some(2))
                .query_params_weight(Some(1))
                .build(),
        ];

        scores.sort();

        // After sorting, exact match should be first (lowest/best score)
        assert!(scores[0].path_exact);
        // Most specific prefix match should be second
        assert_eq!(scores[1].path_weight, Some(10));
        // Least specific should be last
        assert_eq!(scores[2].path_weight, Some(5));
    }

    // Regression tests for specific edge cases
    #[rstest]
    fn test_none_vs_some_zero() {
        let none_weight = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(None)
            .path_prefix(None)
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        let zero_weight = RequestMatchScore::builder()
            .path_exact(false)
            .path_weight(Some(0))
            .path_prefix(Some(Arc::new("".to_string())))
            .method(false)
            .headers_weight(None)
            .query_params_weight(None)
            .build();

        // Some(0) should be better (less than) None
        assert_lt!(zero_weight, none_weight);
        assert_gt!(none_weight, zero_weight);
    }
}
