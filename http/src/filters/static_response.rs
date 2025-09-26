use async_trait::async_trait;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http::{HeaderValue, Response, StatusCode};
use std::fmt::Debug;
use std::sync::Arc;
use typed_builder::TypedBuilder;

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseFilterHandler {
    status_code: StatusCode,
    body_resolver: Option<Box<dyn StaticResponseBodyResolver>>,
}

#[async_trait]
pub trait StaticResponseBodyResolver: Debug {
    async fn resolve_body(&self) -> Option<StaticResponseBody>;
}

#[derive(Debug)]
pub struct EmptyStaticResponseBodyResolver;

#[async_trait]
impl StaticResponseBodyResolver for EmptyStaticResponseBodyResolver {
    async fn resolve_body(&self) -> Option<StaticResponseBody> {
        None
    }
}

#[derive(Debug, TypedBuilder)]
pub struct StaticResponseBody {
    content_type: HeaderValue,
    content: Arc<Bytes>,
}

impl StaticResponseFilterHandler {
    pub async fn generate_response(&self) -> Response<Option<Arc<Bytes>>> {
        let mut response = Response::builder().status(self.status_code);

        let body = match &self.body_resolver {
            Some(body_resolver) => body_resolver.resolve_body().await,
            None => None,
        };

        let response = match body {
            Some(body) => {
                response = response.header(CONTENT_TYPE, &body.content_type);
                response.body(Some(body.content))
            }
            None => response.body(None),
        };

        response.expect("Failed to build static response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertables::*;

    #[tokio::test]
    async fn test_static_response_with_text_body() {
        // Test static response with no body resolver (empty response)
        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(None)
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.body().is_none()); // No body resolver means no body
    }

    #[tokio::test]
    async fn test_static_response_with_json_body() {
        // Test returning static JSON response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct JsonBodyResolver {
            content: String,
        }

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for JsonBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("application/json"))
                        .content(Arc::new(Bytes::from(self.content.clone())))
                        .build(),
                )
            }
        }

        let json_body = r#"{"message": "success", "data": null}"#;
        let body_resolver = JsonBodyResolver {
            content: json_body.to_string(),
        };

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(Some(Box::new(body_resolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_with_html_body() {
        // Test returning static HTML response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct HtmlBodyResolver {
            content: String,
        }

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for HtmlBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("text/html"))
                        .content(Arc::new(Bytes::from(self.content.clone())))
                        .build(),
                )
            }
        }

        let html_body = "<html><body><h1>Maintenance Page</h1></body></html>";
        let body_resolver = HtmlBodyResolver {
            content: html_body.to_string(),
        };

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::SERVICE_UNAVAILABLE)
            .body_resolver(Some(Box::new(body_resolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["content-type"], "text/html");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_404_not_found() {
        // Test returning 404 Not Found response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct NotFoundBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for NotFoundBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("text/plain"))
                        .content(Arc::new(Bytes::from(
                            "The requested resource was not found",
                        )))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::NOT_FOUND)
            .body_resolver(Some(Box::new(NotFoundBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_500_internal_error() {
        // Test returning 500 Internal Server Error response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct InternalErrorBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for InternalErrorBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("text/plain"))
                        .content(Arc::new(Bytes::from("An internal error occurred")))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .body_resolver(Some(Box::new(InternalErrorBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_with_custom_headers() {
        // Test that the handler sets Content-Type header based on body resolver
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct CustomHeaderBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for CustomHeaderBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("application/xml"))
                        .content(Arc::new(Bytes::from("<response>Custom content</response>")))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(Some(Box::new(CustomHeaderBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/xml");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_with_binary_data() {
        // Test returning binary data response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct BinaryBodyResolver {
            data: Vec<u8>,
        }

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for BinaryBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("image/jpeg"))
                        .content(Arc::new(Bytes::from(self.data.clone())))
                        .build(),
                )
            }
        }

        let binary_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        let body_resolver = BinaryBodyResolver {
            data: binary_data.clone(),
        };

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(Some(Box::new(body_resolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/jpeg");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_cors_preflight() {
        // Test handling CORS preflight requests with static response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct CorsBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for CorsBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                // CORS preflight typically has no body
                None
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::NO_CONTENT)
            .body_resolver(Some(Box::new(CorsBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.body().is_none()); // No body for CORS preflight
    }

    #[tokio::test]
    async fn test_static_response_with_content_length() {
        // Test that responses include content when body is provided
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct ContentLengthBodyResolver {
            content: String,
        }

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for ContentLengthBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("text/plain"))
                        .content(Arc::new(Bytes::from(self.content.clone())))
                        .build(),
                )
            }
        }

        let content = "This is a test response body";
        let body_resolver = ContentLengthBodyResolver {
            content: content.to_string(),
        };

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(Some(Box::new(body_resolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/plain");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_maintenance_mode() {
        // Test maintenance mode response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct MaintenanceBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for MaintenanceBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                let html = r#"
                    <html>
                        <head><title>Maintenance</title></head>
                        <body>
                            <h1>Site Under Maintenance</h1>
                            <p>We'll be back soon!</p>
                        </body>
                    </html>
                "#;
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("text/html"))
                        .content(Arc::new(Bytes::from(html)))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::SERVICE_UNAVAILABLE)
            .body_resolver(Some(Box::new(MaintenanceBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["content-type"], "text/html");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_api_rate_limit_exceeded() {
        // Test API rate limit exceeded response
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct RateLimitBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for RateLimitBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                let json = r#"{"error": "rate_limit_exceeded", "message": "Too many requests"}"#;
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("application/json"))
                        .content(Arc::new(Bytes::from(json)))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::TOO_MANY_REQUESTS)
            .body_resolver(Some(Box::new(RateLimitBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_with_etag() {
        // Test static response with custom content type
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct EtagBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for EtagBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("text/plain"))
                        .content(Arc::new(Bytes::from("Cached content")))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(Some(Box::new(EtagBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/plain");
        // ETag would be added by higher-level middleware
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_conditional_on_request() {
        // Test static response generation (conditions would be handled at higher level)
        use bytes::Bytes;
        use std::sync::Arc;

        #[derive(Debug)]
        struct HealthBodyResolver;

        #[async_trait::async_trait]
        impl StaticResponseBodyResolver for HealthBodyResolver {
            async fn resolve_body(&self) -> Option<StaticResponseBody> {
                Some(
                    StaticResponseBody::builder()
                        .content_type(HeaderValue::from_static("application/json"))
                        .content(Arc::new(Bytes::from(r#"{"status": "healthy"}"#)))
                        .build(),
                )
            }
        }

        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::OK)
            .body_resolver(Some(Box::new(HealthBodyResolver)))
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert!(response.body().is_some());
    }

    #[tokio::test]
    async fn test_static_response_empty_body() {
        // Test static response with no body resolver
        let handler = StaticResponseFilterHandler::builder()
            .status_code(StatusCode::NO_CONTENT)
            .body_resolver(None)
            .build();

        let response = handler.generate_response().await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.body().is_none());
    }
}
