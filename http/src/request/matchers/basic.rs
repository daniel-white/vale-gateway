use getset::{CloneGetters, CopyGetters};
use hickory_proto::rr::Name;
use http::HeaderValue;
use regex::Regex;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExactMatcher<T: PartialEq> {
    value: Arc<T>,
}

impl<T: PartialEq> ExactMatcher<T> {
    pub fn new(value: Arc<T>) -> Self {
        Self { value }
    }

    pub fn matches(&self, value: &T) -> bool {
        self.value.as_ref() == value
    }
}

impl ExactMatcher<String> {
    pub fn weight(&self) -> usize {
        self.value.len()
    }

    pub fn matches_str(&self, value: &str) -> bool {
        self.value.as_str() == value
    }
}

impl ExactMatcher<HeaderValue> {
    pub fn weight(&self) -> usize {
        self.value.len()
    }
}

#[derive(Debug, Clone, CloneGetters)]
#[cfg_attr(test, derive(PartialEq))]
pub struct StringPrefixMatcher {
    #[getset(get_clone = "pub")]
    prefix: Arc<String>,
}

impl StringPrefixMatcher {
    pub fn new(prefix: Arc<String>) -> Self {
        Self { prefix }
    }

    pub fn matches(&self, value: &str) -> bool {
        value.starts_with(self.prefix.as_ref())
    }

    pub fn weight(&self) -> usize {
        self.prefix.len()
    }
}

#[derive(Debug, Clone, CopyGetters)]
pub struct RegularExpressionMatcher {
    regex: Arc<Regex>,
    #[getset(get_copy = "pub")]
    weight: usize,
}

impl RegularExpressionMatcher {
    pub fn new(regex: Arc<Regex>) -> Self {
        let weight = regex.as_str().len() * 4;
        Self {
            regex: regex.clone(),
            weight,
        }
    }

    pub fn matches(&self, value: &str) -> bool {
        self.regex.is_match(value)
    }
}

#[cfg(test)]
impl PartialEq for RegularExpressionMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.regex.as_str() == other.regex.as_str()
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExactDnsNameMatcher {
    matcher: ExactMatcher<Name>,
}

impl ExactDnsNameMatcher {
    pub fn new(name: Arc<Name>) -> Self {
        Self {
            matcher: ExactMatcher::new(name),
        }
    }

    pub fn matches(&self, value: &Name) -> bool {
        self.matcher.matches(value)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct InZoneDnsNameMatcher {
    zone: Arc<Name>,
}

impl InZoneDnsNameMatcher {
    pub fn new(zone: Arc<Name>) -> Self {
        Self { zone }
    }

    pub fn matches(&self, value: &Name) -> bool {
        self.zone.zone_of(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::Name;
    use http::HeaderValue;
    use regex::Regex;
    use rstest::*;
    use std::str::FromStr;
    use std::sync::Arc;

    #[cfg(test)]
    mod exact_matcher_tests {
        use super::*;

        #[fixture]
        fn string_matcher() -> ExactMatcher<String> {
            ExactMatcher::new(Arc::new("test".to_string()))
        }

        #[fixture]
        fn header_value_matcher() -> ExactMatcher<HeaderValue> {
            ExactMatcher::new(Arc::new(HeaderValue::from_static("application/json")))
        }

        #[rstest]
        fn test_exact_string_matcher_matches_exact_value(string_matcher: ExactMatcher<String>) {
            assert!(string_matcher.matches(&"test".to_string()));
        }

        #[rstest]
        fn test_exact_string_matcher_doesnt_match_different_value(
            string_matcher: ExactMatcher<String>,
        ) {
            assert!(!string_matcher.matches(&"different".to_string()));
        }

        #[rstest]
        fn test_exact_string_matcher_matches_str(string_matcher: ExactMatcher<String>) {
            assert!(string_matcher.matches_str("test"));
        }

        #[rstest]
        fn test_exact_string_matcher_doesnt_match_str(string_matcher: ExactMatcher<String>) {
            assert!(!string_matcher.matches_str("different"));
        }

        #[rstest]
        fn test_exact_string_matcher_weight(string_matcher: ExactMatcher<String>) {
            assert_eq!(string_matcher.weight(), 4);
        }

        #[rstest]
        #[case("", 0)]
        #[case("a", 1)]
        #[case("hello", 5)]
        #[case("hello world", 11)]
        fn test_exact_string_matcher_weight_with_different_lengths(
            #[case] input: &str,
            #[case] expected: usize,
        ) {
            let matcher = ExactMatcher::new(Arc::new(input.to_string()));
            assert_eq!(matcher.weight(), expected);
        }

        #[rstest]
        fn test_exact_header_value_matcher_matches_exact_value(
            header_value_matcher: ExactMatcher<HeaderValue>,
        ) {
            let header_value = HeaderValue::from_static("application/json");
            assert!(header_value_matcher.matches(&header_value));
        }

        #[rstest]
        fn test_exact_header_value_matcher_doesnt_match_different_value(
            header_value_matcher: ExactMatcher<HeaderValue>,
        ) {
            let header_value = HeaderValue::from_static("text/html");
            assert!(!header_value_matcher.matches(&header_value));
        }

        #[rstest]
        fn test_exact_header_value_matcher_weight(header_value_matcher: ExactMatcher<HeaderValue>) {
            assert_eq!(header_value_matcher.weight(), 16); // "application/json".len()
        }

        #[rstest]
        #[case("text/html", 9)]
        #[case("application/xml", 15)]
        #[case("*/*", 3)]
        fn test_exact_header_value_matcher_weight_with_different_values(
            #[case] input: &str,
            #[case] expected: usize,
        ) {
            let matcher = ExactMatcher::new(Arc::new(HeaderValue::try_from(input).unwrap()));
            assert_eq!(matcher.weight(), expected);
        }

        #[rstest]
        fn test_exact_matcher_case_sensitive() {
            let matcher = ExactMatcher::new(Arc::new("Test".to_string()));
            assert!(!matcher.matches_str("test"));
            assert!(!matcher.matches_str("TEST"));
            assert!(matcher.matches_str("Test"));
        }
    }

    #[cfg(test)]
    mod string_prefix_matcher_tests {
        use super::*;

        #[fixture]
        fn prefix_matcher() -> StringPrefixMatcher {
            StringPrefixMatcher::new(Arc::new("/api".to_string()))
        }

        #[rstest]
        fn test_string_prefix_matcher_matches_exact_prefix(prefix_matcher: StringPrefixMatcher) {
            assert!(prefix_matcher.matches("/api"));
        }

        #[rstest]
        fn test_string_prefix_matcher_matches_with_suffix(prefix_matcher: StringPrefixMatcher) {
            assert!(prefix_matcher.matches("/api/v1"));
            assert!(prefix_matcher.matches("/api/users"));
            assert!(prefix_matcher.matches("/api/"));
        }

        #[rstest]
        fn test_string_prefix_matcher_doesnt_match_different_prefix(
            prefix_matcher: StringPrefixMatcher,
        ) {
            assert!(!prefix_matcher.matches("/web"));
            assert!(!prefix_matcher.matches("/other"));
            assert!(!prefix_matcher.matches("api")); // missing leading slash
        }

        #[rstest]
        fn test_string_prefix_matcher_doesnt_match_partial_prefix(
            prefix_matcher: StringPrefixMatcher,
        ) {
            assert!(!prefix_matcher.matches("/ap"));
            assert!(!prefix_matcher.matches("/a"));
        }

        #[rstest]
        fn test_string_prefix_matcher_weight(prefix_matcher: StringPrefixMatcher) {
            assert_eq!(prefix_matcher.weight(), 4); // "/api".len()
        }

        #[rstest]
        fn test_string_prefix_matcher_prefix_getter(prefix_matcher: StringPrefixMatcher) {
            assert_eq!(prefix_matcher.prefix().as_str(), "/api");
        }

        #[rstest]
        #[case("", 0)]
        #[case("/", 1)]
        #[case("/health", 7)]
        #[case("/api/v1/users", 13)] // 13 characters, not 12
        fn test_string_prefix_matcher_weight_with_different_prefixes(
            #[case] prefix: &str,
            #[case] expected: usize,
        ) {
            let matcher = StringPrefixMatcher::new(Arc::new(prefix.to_string()));
            assert_eq!(matcher.weight(), expected);
        }

        #[rstest]
        fn test_string_prefix_matcher_case_sensitive() {
            let matcher = StringPrefixMatcher::new(Arc::new("/API".to_string()));
            assert!(!matcher.matches("/api"));
            assert!(matcher.matches("/API"));
            assert!(matcher.matches("/API/v1"));
        }

        #[rstest]
        fn test_string_prefix_matcher_empty_prefix() {
            let matcher = StringPrefixMatcher::new(Arc::new("".to_string()));
            assert!(matcher.matches("anything"));
            assert!(matcher.matches(""));
            assert!(matcher.matches("/api"));
        }
    }

    #[cfg(test)]
    mod regular_expression_matcher_tests {
        use super::*;

        #[fixture]
        fn simple_regex_matcher() -> RegularExpressionMatcher {
            let regex = Regex::new(r"^/api/v\d+").unwrap();
            RegularExpressionMatcher::new(Arc::new(regex))
        }

        #[fixture]
        fn complex_regex_matcher() -> RegularExpressionMatcher {
            let regex = Regex::new(r"^/users/[a-zA-Z0-9]+/profile$").unwrap();
            RegularExpressionMatcher::new(Arc::new(regex))
        }

        #[rstest]
        fn test_regex_matcher_matches_pattern(simple_regex_matcher: RegularExpressionMatcher) {
            assert!(simple_regex_matcher.matches("/api/v1"));
            assert!(simple_regex_matcher.matches("/api/v2"));
            assert!(simple_regex_matcher.matches("/api/v123"));
        }

        #[rstest]
        fn test_regex_matcher_doesnt_match_invalid_pattern(
            simple_regex_matcher: RegularExpressionMatcher,
        ) {
            assert!(!simple_regex_matcher.matches("/api/va"));
            assert!(!simple_regex_matcher.matches("/web/v1"));
            assert!(!simple_regex_matcher.matches("api/v1")); // missing leading slash
        }

        #[rstest]
        fn test_regex_matcher_with_complex_pattern(
            complex_regex_matcher: RegularExpressionMatcher,
        ) {
            assert!(complex_regex_matcher.matches("/users/john123/profile"));
            assert!(complex_regex_matcher.matches("/users/A/profile"));
            assert!(complex_regex_matcher.matches("/users/123/profile"));
        }

        #[rstest]
        fn test_regex_matcher_complex_pattern_rejects_invalid(
            complex_regex_matcher: RegularExpressionMatcher,
        ) {
            assert!(!complex_regex_matcher.matches("/users/john-doe/profile")); // hyphen not allowed
            assert!(!complex_regex_matcher.matches("/users/john/profile/settings")); // extra path
            assert!(!complex_regex_matcher.matches("/users//profile")); // empty username
            assert!(!complex_regex_matcher.matches("/users/john/"));
        }

        #[rstest]
        fn test_regex_matcher_weight_calculation(simple_regex_matcher: RegularExpressionMatcher) {
            // Weight should be regex pattern length * 4
            let expected_weight = "^/api/v\\d+".len() * 4; // 10 * 4 = 40
            assert_eq!(simple_regex_matcher.weight(), expected_weight);
        }

        #[rstest]
        fn test_regex_matcher_weight_getter(simple_regex_matcher: RegularExpressionMatcher) {
            assert_eq!(simple_regex_matcher.weight(), 40);
        }

        #[rstest]
        #[case(r"\d+", 12)] // 3 * 4
        #[case(r"^hello$", 28)] // 7 * 4
        #[case(r"[a-zA-Z]+", 36)] // 9 * 4
        fn test_regex_matcher_weight_with_different_patterns(
            #[case] pattern: &str,
            #[case] expected: usize,
        ) {
            let regex = Regex::new(pattern).unwrap();
            let matcher = RegularExpressionMatcher::new(Arc::new(regex));
            assert_eq!(matcher.weight(), expected);
        }

        #[rstest]
        fn test_regex_matcher_case_sensitivity() {
            let regex = Regex::new(r"^/API").unwrap();
            let matcher = RegularExpressionMatcher::new(Arc::new(regex));
            assert!(matcher.matches("/API"));
            assert!(!matcher.matches("/api"));
        }

        #[rstest]
        fn test_regex_matcher_case_insensitive() {
            let regex = Regex::new(r"(?i)^/api").unwrap();
            let matcher = RegularExpressionMatcher::new(Arc::new(regex));
            assert!(matcher.matches("/API"));
            assert!(matcher.matches("/api"));
            assert!(matcher.matches("/Api"));
        }

        #[rstest]
        fn test_regex_matcher_partial_matches() {
            let regex = Regex::new(r"api").unwrap();
            let matcher = RegularExpressionMatcher::new(Arc::new(regex));
            assert!(matcher.matches("/api/v1"));
            assert!(matcher.matches("myapi"));
            assert!(matcher.matches("api"));
        }

        #[rstest]
        fn test_regex_matcher_empty_string() {
            let regex = Regex::new(r".*").unwrap();
            let matcher = RegularExpressionMatcher::new(Arc::new(regex));
            assert!(matcher.matches(""));
            assert!(matcher.matches("anything"));
        }
    }

    #[cfg(test)]
    mod exact_dns_name_matcher_tests {
        use super::*;

        #[fixture]
        fn dns_name_matcher() -> ExactDnsNameMatcher {
            let name = Name::from_str("example.com.").unwrap();
            ExactDnsNameMatcher::new(Arc::new(name))
        }

        #[fixture]
        fn root_dns_name_matcher() -> ExactDnsNameMatcher {
            let name = Name::root();
            ExactDnsNameMatcher::new(Arc::new(name))
        }

        #[rstest]
        fn test_exact_dns_name_matcher_matches_exact_name(dns_name_matcher: ExactDnsNameMatcher) {
            let name = Name::from_str("example.com.").unwrap();
            assert!(dns_name_matcher.matches(&name));
        }

        #[rstest]
        fn test_exact_dns_name_matcher_doesnt_match_different_name(
            dns_name_matcher: ExactDnsNameMatcher,
        ) {
            let name = Name::from_str("other.com.").unwrap();
            assert!(!dns_name_matcher.matches(&name));
        }

        #[rstest]
        fn test_exact_dns_name_matcher_doesnt_match_subdomain(
            dns_name_matcher: ExactDnsNameMatcher,
        ) {
            let name = Name::from_str("sub.example.com.").unwrap();
            assert!(!dns_name_matcher.matches(&name));
        }

        #[rstest]
        fn test_exact_dns_name_matcher_doesnt_match_parent_domain(
            dns_name_matcher: ExactDnsNameMatcher,
        ) {
            let name = Name::from_str("com.").unwrap();
            assert!(!dns_name_matcher.matches(&name));
        }

        #[rstest]
        fn test_exact_dns_name_matcher_root_domain(root_dns_name_matcher: ExactDnsNameMatcher) {
            let root_name = Name::root();
            assert!(root_dns_name_matcher.matches(&root_name));
        }

        #[rstest]
        fn test_exact_dns_name_matcher_root_doesnt_match_other(
            root_dns_name_matcher: ExactDnsNameMatcher,
        ) {
            let name = Name::from_str("example.com.").unwrap();
            assert!(!root_dns_name_matcher.matches(&name));
        }

        #[rstest]
        #[case("google.com.")]
        #[case("sub.domain.example.org.")]
        #[case("localhost.")]
        fn test_exact_dns_name_matcher_with_various_names(#[case] dns_name: &str) {
            let name = Name::from_str(dns_name).unwrap();
            let matcher = ExactDnsNameMatcher::new(Arc::new(name.clone()));
            assert!(matcher.matches(&name));

            // Should not match a different name
            let other_name = Name::from_str("different.com.").unwrap();
            assert!(!matcher.matches(&other_name));
        }

        #[rstest]
        fn test_exact_dns_name_matcher_case_insensitive() {
            // DNS names should be case-insensitive
            let lower_name = Name::from_str("example.com.").unwrap();
            let upper_name = Name::from_str("EXAMPLE.COM.").unwrap();
            let mixed_name = Name::from_str("Example.Com.").unwrap();

            let matcher = ExactDnsNameMatcher::new(Arc::new(lower_name.clone()));

            assert!(matcher.matches(&lower_name));
            assert!(matcher.matches(&upper_name));
            assert!(matcher.matches(&mixed_name));
        }
    }

    #[cfg(test)]
    mod in_zone_dns_name_matcher_tests {
        use super::*;

        #[fixture]
        fn zone_matcher() -> InZoneDnsNameMatcher {
            let zone = Name::from_str("example.com.").unwrap();
            InZoneDnsNameMatcher::new(Arc::new(zone))
        }

        #[fixture]
        fn root_zone_matcher() -> InZoneDnsNameMatcher {
            let zone = Name::root();
            InZoneDnsNameMatcher::new(Arc::new(zone))
        }

        #[rstest]
        fn test_in_zone_matcher_matches_exact_zone(zone_matcher: InZoneDnsNameMatcher) {
            let name = Name::from_str("example.com.").unwrap();
            assert!(zone_matcher.matches(&name));
        }

        #[rstest]
        fn test_in_zone_matcher_matches_subdomain(zone_matcher: InZoneDnsNameMatcher) {
            let subdomain = Name::from_str("sub.example.com.").unwrap();
            assert!(zone_matcher.matches(&subdomain));

            let deep_subdomain = Name::from_str("deep.sub.example.com.").unwrap();
            assert!(zone_matcher.matches(&deep_subdomain));
        }

        #[rstest]
        fn test_in_zone_matcher_doesnt_match_parent_domain(zone_matcher: InZoneDnsNameMatcher) {
            let parent = Name::from_str("com.").unwrap();
            assert!(!zone_matcher.matches(&parent));
        }

        #[rstest]
        fn test_in_zone_matcher_doesnt_match_different_zone(zone_matcher: InZoneDnsNameMatcher) {
            let other_zone = Name::from_str("other.com.").unwrap();
            assert!(!zone_matcher.matches(&other_zone));

            let similar_zone = Name::from_str("notexample.com.").unwrap();
            assert!(!zone_matcher.matches(&similar_zone));
        }

        #[rstest]
        fn test_in_zone_matcher_root_zone_matches_everything(
            root_zone_matcher: InZoneDnsNameMatcher,
        ) {
            let any_name = Name::from_str("anything.example.com.").unwrap();
            assert!(root_zone_matcher.matches(&any_name));

            let tld = Name::from_str("com.").unwrap();
            assert!(root_zone_matcher.matches(&tld));

            let root = Name::root();
            assert!(root_zone_matcher.matches(&root));
        }

        #[rstest]
        #[case("example.org.", "sub.example.org.", true)]
        #[case("example.org.", "deep.sub.example.org.", true)]
        #[case("example.org.", "example.org.", true)]
        #[case("example.org.", "other.org.", false)]
        #[case("example.org.", "org.", false)]
        #[case("sub.example.org.", "deep.sub.example.org.", true)]
        #[case("sub.example.org.", "example.org.", false)]
        fn test_in_zone_matcher_various_combinations(
            #[case] zone_name: &str,
            #[case] test_name: &str,
            #[case] expected: bool,
        ) {
            let zone = Name::from_str(zone_name).unwrap();
            let name = Name::from_str(test_name).unwrap();
            let matcher = InZoneDnsNameMatcher::new(Arc::new(zone));

            assert_eq!(matcher.matches(&name), expected);
        }

        #[rstest]
        fn test_in_zone_matcher_case_insensitive() {
            let zone = Name::from_str("Example.Com.").unwrap();
            let matcher = InZoneDnsNameMatcher::new(Arc::new(zone));

            let lower_subdomain = Name::from_str("sub.example.com.").unwrap();
            let upper_subdomain = Name::from_str("SUB.EXAMPLE.COM.").unwrap();
            let mixed_subdomain = Name::from_str("Sub.Example.Com.").unwrap();

            assert!(matcher.matches(&lower_subdomain));
            assert!(matcher.matches(&upper_subdomain));
            assert!(matcher.matches(&mixed_subdomain));
        }

        #[rstest]
        fn test_in_zone_matcher_single_label() {
            // Test with single label domain (like localhost.)
            let zone = Name::from_str("localhost.").unwrap();
            let matcher = InZoneDnsNameMatcher::new(Arc::new(zone));

            let exact_match = Name::from_str("localhost.").unwrap();
            assert!(matcher.matches(&exact_match));

            // Single label domains don't have subdomains in the traditional sense
            let other_single = Name::from_str("other.").unwrap();
            assert!(!matcher.matches(&other_single));
        }
    }
}
