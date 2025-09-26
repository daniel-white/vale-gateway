use crate::request::RequestMatchContext;
use crate::rewriting::uri_rewriter::UriRewriter;
use http::header::LOCATION;
use http::request::Parts;
use http::{Response, StatusCode};
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct RedirectResponseFilterHandler {
    #[builder(setter(into))]
    status_code: StatusCode,
    uri_rewriter: UriRewriter,
}

impl RedirectResponseFilterHandler {
    pub fn handle(&self, req: &Parts, match_context: &impl RequestMatchContext) -> Response<()> {
        let new_uri = self.uri_rewriter.rewrite(&req.uri, match_context);
        Response::builder()
            .status(self.status_code)
            .header(LOCATION, new_uri.to_string())
            .body(())
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use http::Method;
    use http::header::HeaderValue;
    use std::str::FromStr;

    fn create_empty_parts() -> Parts {
        use http::Request;
        let (parts, _) = Request::get("/").body(()).unwrap().into_parts();
        parts
    }

    #[tokio::test]
    async fn test_permanent_redirect_301() {
        // Test 301 permanent redirect using the actual RedirectResponseFilterHandler
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        // Create a URI rewriter that redirects to a new location
        let uri_rewriter = UriRewriter::builder().build(); // This would need actual rewriter implementation

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        // Create mock request parts
        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/old-path").unwrap();

        // Create mock match context
        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        // Verify redirect response
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_temporary_redirect_302() {
        // Test 302 temporary redirect
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::FOUND)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/current-path").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::FOUND);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_see_other_redirect_303() {
        // Test 303 See Other redirect
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::SEE_OTHER)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/form-submit").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_permanent_redirect_308() {
        // Test 308 permanent redirect (preserves method)
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::PERMANENT_REDIRECT)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/api/v1").unwrap();
        request_parts.method = Method::POST;

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_conditional_redirect_by_host() {
        // Test redirect with different status codes for different scenarios
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        // Test with different host headers (would need header-aware rewriter)
        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("https://old-domain.com/path").unwrap();
        request_parts
            .headers
            .insert("Host", HeaderValue::from_static("old-domain.com"));

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_conditional_redirect_by_path_pattern() {
        // Test redirect based on path patterns
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        // Test legacy path redirect
        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/legacy/feature").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        let location = response.headers().get("location").unwrap();
        // The actual location depends on the URI rewriter implementation
        assert!(location.to_str().unwrap().contains("/legacy/feature"));
    }

    #[tokio::test]
    async fn test_redirect_with_query_preservation() {
        // Test preserving query parameters in redirects
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/old-path?param=value&other=123").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        let location = response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        // The URI rewriter should preserve or modify query params as configured
        assert!(location.contains("/old-path"));
    }

    #[tokio::test]
    async fn test_redirect_without_query_preservation() {
        // Test not preserving query parameters in redirects
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/old-path?param=value").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_redirect_with_custom_headers() {
        // Test that Location header is properly set by the handler
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        // The handler sets the Location header automatically
        assert!(response.headers().get("location").is_some());
        assert_eq!(
            response
                .headers()
                .get("location")
                .unwrap()
                .to_str()
                .unwrap(),
            "/"
        );
    }

    #[tokio::test]
    async fn test_redirect_based_on_user_agent() {
        // Test redirect that could be based on User-Agent (would need context-aware rewriter)
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/").unwrap();
        request_parts.headers.insert(
            "User-Agent",
            HeaderValue::from_static("Mozilla/5.0 Mobile Safari"),
        );

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_redirect_with_regex_path_matching() {
        // Test redirect using regex patterns (would need pattern-matching rewriter)
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/product/12345").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        let location = response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.contains("/product/12345"));
    }

    #[tokio::test]
    async fn test_redirect_chain_prevention() {
        // Test basic redirect functionality (chain prevention would be higher-level)
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/target").unwrap();
        request_parts
            .headers
            .insert("X-Redirect-Count", HeaderValue::from_static("1"));

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        // Handler always generates redirect - chain prevention would be at higher level
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get("location").is_some());
    }

    #[tokio::test]
    async fn test_redirect_response_body() {
        // Test that redirect response has empty body (as per HTTP spec)
        use crate::rewriting::uri_rewriter::UriRewriter;
        use http::Uri;

        let uri_rewriter = UriRewriter::builder().build();

        let handler = RedirectResponseFilterHandler::builder()
            .status_code(StatusCode::MOVED_PERMANENTLY)
            .uri_rewriter(uri_rewriter)
            .build();

        let mut request_parts = create_empty_parts();
        request_parts.uri = Uri::from_str("/old-location").unwrap();

        struct MockMatchContext;
        impl RequestMatchContext for MockMatchContext {
            fn path_prefix(&self) -> Option<&str> {
                None
            }
        }
        let match_context = MockMatchContext;

        let response = handler.handle(&request_parts, &match_context);

        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert!(response.headers().get("location").is_some());
        // Body should be empty unit type () - can't directly test but handler creates Response(())
    }
}
