use crate::header::HeaderModifier;
use http::{HeaderMap, HeaderName};
use std::collections::HashSet;
use thiserror::Error;
use typed_builder::TypedBuilder;
use vg_http_config::filters::header_modifier::HeaderModifierFilter;

#[derive(Debug, TypedBuilder)]
pub struct HeaderModifierFilterHandler {
    add: HeaderMap,
    set: HeaderMap,
    remove: HashSet<HeaderName>,
}

impl HeaderModifierFilterHandler {
    pub fn apply(&self, modifier: &mut impl HeaderModifier) {
        // Remove headers first
        for name in &self.remove {
            modifier.remove(name);
        }

        // Set headers (overwrite existing)
        for (name, value) in &self.set {
            modifier.insert(name, value);
        }

        // Add headers (append to existing)
        for (name, value) in &self.add {
            modifier.append(name, value);
        }
    }
}

#[derive(Debug, Error)]
pub enum HeaderModifierFilterConversionError {}

#[allow(clippy::infallible_try_from)]
impl TryFrom<&HeaderModifierFilter> for HeaderModifierFilterHandler {
    type Error = HeaderModifierFilterConversionError;

    fn try_from(value: &HeaderModifierFilter) -> Result<Self, Self::Error> {
        let handler = Self::builder()
            .add(value.add())
            .set(value.set())
            .remove(value.remove().into_iter().collect())
            .build();

        Ok(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[tokio::test]
    async fn test_header_addition() {
        // Test adding headers to requests/responses
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Custom-Header", HeaderValue::from_static("custom-value"));
        add_headers.insert("X-Gateway-Version", HeaderValue::from_static("1.0"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(add_headers)
            .set(HeaderMap::new())
            .remove(HashSet::new())
            .build();

        // Create a mock modifier to test with
        let mut test_headers = HeaderMap::new();
        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify headers were added
        assert_eq!(test_headers.get("x-custom-header").unwrap(), "custom-value");
        assert_eq!(test_headers.get("x-gateway-version").unwrap(), "1.0");
    }

    #[tokio::test]
    async fn test_header_removal() {
        // Test removing headers from requests/responses
        let mut remove_headers = HashSet::new();
        remove_headers.insert(HeaderName::from_static("server"));
        remove_headers.insert(HeaderName::from_static("x-powered-by"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(HeaderMap::new())
            .set(HeaderMap::new())
            .remove(remove_headers)
            .build();

        // Start with headers that should be removed
        let mut test_headers = HeaderMap::new();
        test_headers.insert("Server", HeaderValue::from_static("nginx/1.0"));
        test_headers.insert("X-Powered-By", HeaderValue::from_static("PHP/7.4"));
        test_headers.insert("Content-Type", HeaderValue::from_static("text/html"));

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify specified headers were removed
        assert!(test_headers.get("server").is_none());
        assert!(test_headers.get("x-powered-by").is_none());
        // Verify other headers remain
        assert_eq!(test_headers.get("content-type").unwrap(), "text/html");
    }

    #[tokio::test]
    async fn test_header_modification() {
        // Test modifying existing headers using set operation
        let mut set_headers = HeaderMap::new();
        set_headers.insert("User-Agent", HeaderValue::from_static("Gateway-Proxy/1.0"));
        set_headers.insert("Server", HeaderValue::from_static("Vale-Gateway"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(HeaderMap::new())
            .set(set_headers)
            .remove(HashSet::new())
            .build();

        // Start with existing headers
        let mut test_headers = HeaderMap::new();
        test_headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0"));
        test_headers.insert("Content-Type", HeaderValue::from_static("text/html"));

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify headers were modified
        assert_eq!(test_headers.get("user-agent").unwrap(), "Gateway-Proxy/1.0");
        assert_eq!(test_headers.get("server").unwrap(), "Vale-Gateway");
        // Verify unmodified headers remain
        assert_eq!(test_headers.get("content-type").unwrap(), "text/html");
    }

    #[tokio::test]
    async fn test_header_conditional_addition() {
        // Test conditionally adding headers - using append which adds without overwriting
        let mut add_headers = HeaderMap::new();
        add_headers.insert(
            "X-Request-ID",
            HeaderValue::from_static("auto-generated-123"),
        );

        let handler = HeaderModifierFilterHandler::builder()
            .add(add_headers)
            .set(HeaderMap::new())
            .remove(HashSet::new())
            .build();

        // Test with headers already present (append behavior)
        let mut test_headers = HeaderMap::new();
        test_headers.insert("X-Request-ID", HeaderValue::from_static("existing-456"));

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // With append, both values should be present
        let values: Vec<_> = test_headers.get_all("x-request-id").iter().collect();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&&HeaderValue::from_static("existing-456")));
        assert!(values.contains(&&HeaderValue::from_static("auto-generated-123")));
    }

    #[tokio::test]
    async fn test_header_security_headers() {
        // Test adding common security headers
        let mut security_headers = HeaderMap::new();
        security_headers.insert(
            "X-Content-Type-Options",
            HeaderValue::from_static("nosniff"),
        );
        security_headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
        security_headers.insert(
            "X-XSS-Protection",
            HeaderValue::from_static("1; mode=block"),
        );
        security_headers.insert(
            "Strict-Transport-Security",
            HeaderValue::from_static("max-age=31536000"),
        );

        let handler = HeaderModifierFilterHandler::builder()
            .add(security_headers)
            .set(HeaderMap::new())
            .remove(HashSet::new())
            .build();

        let mut test_headers = HeaderMap::new();

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify security headers were added
        assert_eq!(
            test_headers.get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(test_headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            test_headers.get("x-xss-protection").unwrap(),
            "1; mode=block"
        );
        assert_eq!(
            test_headers.get("strict-transport-security").unwrap(),
            "max-age=31536000"
        );
    }

    #[tokio::test]
    async fn test_header_cors_headers() {
        // Test adding CORS headers
        let mut cors_headers = HeaderMap::new();
        cors_headers.insert(
            "Access-Control-Allow-Origin",
            HeaderValue::from_static("https://example.com"),
        );
        cors_headers.insert(
            "Access-Control-Allow-Methods",
            HeaderValue::from_static("GET, POST, PUT, DELETE"),
        );
        cors_headers.insert(
            "Access-Control-Allow-Headers",
            HeaderValue::from_static("Content-Type, Authorization"),
        );
        cors_headers.insert("Access-Control-Max-Age", HeaderValue::from_static("86400"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(cors_headers)
            .set(HeaderMap::new())
            .remove(HashSet::new())
            .build();

        let mut test_headers = HeaderMap::new();

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify CORS headers were added
        assert_eq!(
            test_headers.get("access-control-allow-origin").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            test_headers.get("access-control-allow-methods").unwrap(),
            "GET, POST, PUT, DELETE"
        );
        assert_eq!(
            test_headers.get("access-control-allow-headers").unwrap(),
            "Content-Type, Authorization"
        );
        assert_eq!(test_headers.get("access-control-max-age").unwrap(), "86400");
    }

    #[tokio::test]
    async fn test_header_forwarded_headers() {
        // Test handling X-Forwarded headers
        let mut forwarded_headers = HeaderMap::new();
        forwarded_headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.1"));
        forwarded_headers.insert("X-Forwarded-Proto", HeaderValue::from_static("https"));
        forwarded_headers.insert("X-Forwarded-Host", HeaderValue::from_static("example.com"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(forwarded_headers)
            .set(HeaderMap::new())
            .remove(HashSet::new())
            .build();

        let mut test_headers = HeaderMap::new();

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify X-Forwarded headers were added
        assert_eq!(test_headers.get("x-forwarded-for").unwrap(), "192.168.1.1");
        assert_eq!(test_headers.get("x-forwarded-proto").unwrap(), "https");
        assert_eq!(test_headers.get("x-forwarded-host").unwrap(), "example.com");
    }

    #[tokio::test]
    async fn test_header_sanitization() {
        // Test sanitizing potentially dangerous headers by removing them
        let mut remove_headers = HashSet::new();
        remove_headers.insert(HeaderName::from_static("x-original-ip"));
        remove_headers.insert(HeaderName::from_static("x-real-ip")); // Remove potentially spoofed headers

        let handler = HeaderModifierFilterHandler::builder()
            .add(HeaderMap::new())
            .set(HeaderMap::new())
            .remove(remove_headers)
            .build();

        let mut test_headers = HeaderMap::new();
        test_headers.insert("X-Original-IP", HeaderValue::from_static("192.168.1.1"));
        test_headers.insert("X-Real-IP", HeaderValue::from_static("10.0.0.1"));
        test_headers.insert("Authorization", HeaderValue::from_static("Bearer token"));

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Should preserve legitimate headers
        assert!(test_headers.get("authorization").is_some());
        // Should remove potentially spoofed headers
        assert!(test_headers.get("x-original-ip").is_none());
        assert!(test_headers.get("x-real-ip").is_none());
    }

    #[tokio::test]
    async fn test_header_case_insensitive_operations() {
        // Test that header operations work with case variations
        let mut remove_headers = HashSet::new();
        remove_headers.insert(HeaderName::from_static("content-type"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(HeaderMap::new())
            .set(HeaderMap::new())
            .remove(remove_headers)
            .build();

        let mut test_headers = HeaderMap::new();
        test_headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Header should be removed (HTTP headers are case-insensitive)
        assert!(test_headers.get("content-type").is_none());
    }

    #[tokio::test]
    async fn test_header_multiple_values() {
        // Test handling headers with multiple values using append
        let mut add_headers = HeaderMap::new();
        add_headers.append("Cache-Control", HeaderValue::from_static("no-cache"));
        add_headers.append("Cache-Control", HeaderValue::from_static("no-store"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(add_headers)
            .set(HeaderMap::new())
            .remove(HashSet::new())
            .build();

        let mut test_headers = HeaderMap::new();

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Should have Cache-Control values
        let values: Vec<_> = test_headers.get_all("cache-control").iter().collect();
        assert!(!values.is_empty());
        // Should have at least one of the Cache-Control directives
        let values_str: Vec<String> = values
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect();
        assert!(
            values_str.iter().any(|v| v.contains("no-cache"))
                || values_str.iter().any(|v| v.contains("no-store"))
        );
    }

    #[tokio::test]
    async fn test_header_chained_operations() {
        // Test chaining multiple header operations (remove, set, add)
        let mut add_headers = HeaderMap::new();
        add_headers.insert("X-Custom-Header", HeaderValue::from_static("value1"));

        let mut set_headers = HeaderMap::new();
        set_headers.insert("User-Agent", HeaderValue::from_static("Gateway/1.0"));

        let mut remove_headers = HashSet::new();
        remove_headers.insert(HeaderName::from_static("server"));

        let handler = HeaderModifierFilterHandler::builder()
            .add(add_headers)
            .set(set_headers)
            .remove(remove_headers)
            .build();

        let mut test_headers = HeaderMap::new();
        test_headers.insert("Server", HeaderValue::from_static("nginx/1.0"));
        test_headers.insert("User-Agent", HeaderValue::from_static("curl/7.0"));
        test_headers.insert("Content-Type", HeaderValue::from_static("text/html"));

        struct MockModifier<'a> {
            headers: &'a mut HeaderMap,
        }

        impl<'a> HeaderModifier for MockModifier<'a> {
            fn remove(&mut self, name: &HeaderName) {
                self.headers.remove(name);
            }

            fn insert(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.insert(name, value.clone());
            }

            fn append(&mut self, name: &HeaderName, value: &HeaderValue) {
                self.headers.append(name, value.clone());
            }
        }

        let mut modifier = MockModifier {
            headers: &mut test_headers,
        };
        handler.apply(&mut modifier);

        // Verify operations were applied in order: remove, set, add
        assert!(test_headers.get("x-custom-header").is_some()); // Added
        assert!(test_headers.get("server").is_none()); // Removed
        assert_eq!(test_headers.get("user-agent").unwrap(), "Gateway/1.0"); // Set (overwritten)
        assert_eq!(test_headers.get("content-type").unwrap(), "text/html"); // Unchanged
    }
}
