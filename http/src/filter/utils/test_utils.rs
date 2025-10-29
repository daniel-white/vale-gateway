//! Testing utilities for filter implementations
//!
//! This module provides common utilities and helpers for testing filter
//! implementations, including mock services, test request builders, and
//! assertion helpers.

use crate::filter::{FilterRequest, FilterResponse};
use http::{Request, Response, StatusCode};
use std::net::IpAddr;
use tower::{Service, ServiceExt};

/// Creates a basic test request with common extensions
pub fn create_test_request() -> FilterRequest {
    Request::builder()
        .uri("/test")
        .extension(crate::extensions::ClientIp::new(
            "192.168.1.1".parse().unwrap(),
        ))
        .body(())
        .unwrap()
}

/// Creates a test request with a specific client IP
pub fn create_test_request_with_ip(ip: IpAddr) -> FilterRequest {
    Request::builder()
        .uri("/test")
        .extension(crate::extensions::ClientIp::new(ip))
        .body(())
        .unwrap()
}

/// Creates a test request with custom headers
pub fn create_test_request_with_headers(headers: &[(&str, &str)]) -> FilterRequest {
    let mut builder = Request::builder().uri("/test");

    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }

    builder
        .extension(crate::extensions::ClientIp::new(
            "192.168.1.1".parse().unwrap(),
        ))
        .body(())
        .unwrap()
}

/// Creates a mock service that always returns OK
pub fn create_mock_service() -> impl Service<
    FilterRequest,
    Response = FilterResponse,
    Error = Box<dyn std::error::Error + Send + Sync>,
> + Clone {
    tower::service_fn(|_req| async move { Ok(Response::new(())) })
}

/// Creates a mock service that returns a specific status code
pub fn create_mock_service_with_status(
    status: StatusCode,
) -> impl Service<
    FilterRequest,
    Response = FilterResponse,
    Error = Box<dyn std::error::Error + Send + Sync>,
> + Clone {
    tower::service_fn(move |_req| async move {
        Ok(Response::builder().status(status).body(()).unwrap())
    })
}

/// Asserts that a filter produces the expected response status
pub async fn assert_filter_status<F>(
    mut filter: F,
    input: FilterRequest,
    expected_status: StatusCode,
) where
    F: Service<FilterRequest, Response = FilterResponse> + Send,
    F::Error: std::fmt::Debug,
{
    let response = filter.ready().await.unwrap().call(input).await.unwrap();
    assert_eq!(response.status(), expected_status);
}

/// Asserts that a filter produces an error
pub async fn assert_filter_error<F>(mut filter: F, input: FilterRequest)
where
    F: Service<FilterRequest, Response = FilterResponse> + Send,
    F::Error: std::fmt::Debug,
{
    let result = filter.ready().await.unwrap().call(input).await;
    assert!(result.is_err());
}

/// Asserts that a response contains a specific header
pub fn assert_header_exists(response: &FilterResponse, header_name: &str) {
    assert!(
        response.headers().contains_key(header_name),
        "Expected header '{}' not found in response",
        header_name
    );
}

/// Asserts that a response contains a specific header with a specific value
pub fn assert_header_value(response: &FilterResponse, header_name: &str, expected_value: &str) {
    let header_value = response
        .headers()
        .get(header_name)
        .unwrap_or_else(|| panic!("Header '{}' not found in response", header_name))
        .to_str()
        .unwrap();

    assert_eq!(
        header_value, expected_value,
        "Header '{}' has value '{}', expected '{}'",
        header_name, header_value, expected_value
    );
}

/// Asserts that a response does not contain a specific header
pub fn assert_header_not_exists(response: &FilterResponse, header_name: &str) {
    assert!(
        !response.headers().contains_key(header_name),
        "Unexpected header '{}' found in response",
        header_name
    );
}
