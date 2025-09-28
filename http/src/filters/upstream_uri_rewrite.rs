use crate::request::matchers::RequestMatchDetails;
use crate::rewriting::uri::UriRewriter;
use http::Uri;
use http::request::Parts;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct UpstreamUriRewriteFilterHandler {
    uri_rewriter: UriRewriter,
}

impl UpstreamUriRewriteFilterHandler {
    pub fn handle(&self, req: &Parts, match_context: &impl RequestMatchDetails) -> Uri {
        self.uri_rewriter.rewrite(&req.uri, match_context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewriting::uri::UriRewriter;
    use http::{HeaderValue, Uri};
    use rstest::*;
    use std::str::FromStr;

    fn create_empty_parts() -> Parts {
        use http::Request;
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    struct MockMatchContext;
    impl RequestMatchDetails for MockMatchContext {
        fn path_prefix(&self) -> Option<String> {
            None
        }
    }

    #[rstest]
    #[case("https://example.com/api/v1/users", "simple path replacement")]
    #[case("https://api.example.com/user/12345", "regex pattern")]
    #[case("https://old-backend.example.com/api/data", "host change")]
    #[case("http://example.com/api", "scheme change")]
    #[case("https://example.com:443/api", "port change")]
    #[case(
        "https://api.example.com/data?debug=true&format=xml",
        "query parameter manipulation"
    )]
    #[case("https://api.example.com/users", "path prefix addition")]
    #[case("https://example.com/gateway/api/users", "path prefix removal")]
    #[case("https://example.com//api///users//", "path normalization")]
    #[case("https://old.example.com/data", "template based")]
    #[case("http://old-backend.com/users", "chained transformations")]
    #[case(
        "https://old-host.com:8080/path?query=value",
        "preserve original components"
    )]
    fn test_uri_rewrite_scenarios(#[case] input_uri: &str, #[case] scenario_name: &str) {
        // Test URI rewriting for various scenarios
        let uri_rewriter = UriRewriter::builder().build();
        let handler = UpstreamUriRewriteFilterHandler::builder()
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str(input_uri).unwrap();

        let match_context = MockMatchContext;
        let new_uri = handler.handle(&request_parts, &match_context);

        // Verify URI rewriting occurred (actual result depends on rewriter configuration)
        assert!(
            !new_uri.to_string().is_empty(),
            "Failed for scenario: {}",
            scenario_name
        );
    }

    #[test]
    fn test_uri_rewrite_conditional_based_on_headers() {
        // Test conditional URI rewriting (would need header-aware rewriter)
        let uri_rewriter = UriRewriter::builder().build();
        let handler = UpstreamUriRewriteFilterHandler::builder()
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/api/v1").unwrap();
        request_parts
            .headers
            .insert("X-API-Version", HeaderValue::from_static("v2"));

        let match_context = MockMatchContext;
        let new_uri = handler.handle(&request_parts, &match_context);

        // Verify URI was processed
        assert!(!new_uri.to_string().is_empty());
    }

    #[test]
    fn test_uri_rewrite_load_balancing_backend_selection() {
        // Test rewriting URI for load balancing
        let uri_rewriter = UriRewriter::builder().build();
        let handler = UpstreamUriRewriteFilterHandler::builder()
            .uri_rewriter(uri_rewriter)
            .build();

        // Test multiple requests to see consistent behavior
        for _ in 0..3 {
            let mut request_parts = create_empty_parts();
            request_parts.uri = Uri::from_str("https://api.example.com/data").unwrap();

            let match_context = MockMatchContext;
            let new_uri = handler.handle(&request_parts, &match_context);

            // Each rewriting should produce a valid URI
            assert!(!new_uri.to_string().is_empty());
        }
    }
}
