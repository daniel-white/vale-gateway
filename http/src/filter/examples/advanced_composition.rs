//! Advanced composition examples for the filter system
//!
//! This module demonstrates complex filter compositions and advanced usage patterns.

#![allow(unused)]

use crate::filter::{
    FilterCollection, FilterServiceFactory, StageServices,
    handlers::{
        AccessControlFilterHandler, BackendUriRewriterFilterHandler, HeaderModifierFilterHandler,
        RedirectResponseFilterHandler, StaticResponseFilterHandler,
    },
    layer::{BackendRequestLayer, InboundRequestLayer, PreBackendLayer},
};
use http::{HeaderMap, StatusCode};
use std::collections::HashSet;
use tower::{ServiceBuilder, ServiceExt};
use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
use vg_config::http::filter::backend_uri_rewriter::BackendUriRewriterFilter;
use vg_config::http::filter::header_modifier::HeaderModifierFilter;
use vg_config::http::filter::redirect_response::RedirectResponseFilter;
use vg_config::http::filter::static_response::StaticResponseFilter;
use vg_config::http::rewriting::uri::UriRewriter;
use vg_core::net::IpRef;

/// Example: Complex multi-stage filter pipeline
pub async fn complex_pipeline_example() -> Result<(), Box<dyn std::error::Error>> {
    // Stage 1: Inbound request filtering (access control)
    let access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![
            IpRef::Net("192.168.0.0/16".parse()?),
            IpRef::Net("10.0.0.0/8".parse()?),
        ])
        .build();
    let access_handler = AccessControlFilterHandler::try_from(access_config)?;

    // Stage 2: Pre-backend filtering (header modification)
    let mut request_headers = HeaderMap::new();
    request_headers.insert("x-forwarded-by", "vale-gateway".parse()?);
    request_headers.insert("x-request-id", "req-12345".parse()?);

    let mut remove_headers = HashSet::new();
    remove_headers.insert("x-internal-header".parse()?);

    let header_config = HeaderModifierFilter::builder()
        .add(request_headers)
        .remove(remove_headers)
        .build();
    let header_handler = HeaderModifierFilterHandler::try_from(header_config)?;

    // Stage 3: Backend request filtering (URI rewriting)
    let rewriter = UriRewriter::builder()
        .path_prefix("/api/v1".to_string())
        .build();

    let uri_config = BackendUriRewriterFilter::builder()
        .rewriter(rewriter)
        .build();
    let uri_handler = BackendUriRewriterFilterHandler::try_from(uri_config)?;

    // Build comprehensive filter collection
    let collection = FilterCollection::builder()
        .add_inbound_request(access_handler)
        .add_pre_backend(header_handler)
        .add_backend_request(uri_handler)
        .build();

    // Create services for all stages
    let services = collection.build_services()?;

    // Process request through all stages
    let request = crate::filter::utils::test_utils::create_test_request();

    // Stage 1: Inbound processing
    let stage1_response = services.inbound_request.oneshot(request).await?;
    println!("Stage 1 (inbound) completed");

    // Stage 2: Pre-backend processing
    let stage2_request = http::Request::new(());
    let stage2_response = services.pre_backend.oneshot(stage2_request).await?;
    println!("Stage 2 (pre-backend) completed");

    // Stage 3: Backend request processing
    let stage3_request = http::Request::new(());
    let stage3_response = services.backend_request.oneshot(stage3_request).await?;
    println!("Stage 3 (backend request) completed");

    Ok(())
}

/// Example: Conditional filter application based on request properties
pub async fn conditional_filtering_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create different filter configurations for different scenarios

    // Admin access: More permissive
    let admin_access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![
            IpRef::Net("192.168.1.0/24".parse()?), // Admin network
            IpRef::Addr("203.0.113.1".parse()?),   // Admin IP
        ])
        .build();

    // Public access: More restrictive
    let public_access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Deny)
        .clients(vec![
            IpRef::Net("10.0.0.0/8".parse()?), // Internal network
        ])
        .build();

    // Create handlers
    let admin_handler = AccessControlFilterHandler::try_from(admin_access_config)?;
    let public_handler = AccessControlFilterHandler::try_from(public_access_config)?;

    // Simulate conditional application (in real usage, this would be based on request analysis)
    let is_admin_request = true; // This would be determined by request analysis

    let selected_handler = if is_admin_request {
        admin_handler
    } else {
        public_handler
    };

    // Apply the selected filter
    let request =
        crate::filter::utils::test_utils::create_test_request_with_ip("192.168.1.100".parse()?);
    let response = selected_handler.oneshot(request).await?;

    println!("Conditional filtering applied successfully");
    Ok(())
}

/// Example: Error handling and fallback responses
pub async fn error_handling_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create a static response filter for error scenarios
    let error_response_config = StaticResponseFilter::builder()
        .status_code(StatusCode::SERVICE_UNAVAILABLE)
        .body("Service temporarily unavailable".to_string())
        .build();
    let error_handler = StaticResponseFilterHandler::try_from(error_response_config)?;

    // Create a redirect filter for maintenance scenarios
    let redirect_config = RedirectResponseFilter::builder()
        .status_code(StatusCode::TEMPORARY_REDIRECT)
        .location("https://maintenance.example.com".to_string())
        .build();
    let redirect_handler = RedirectResponseFilterHandler::try_from(redirect_config)?;

    // Build collection with error handling filters
    let collection = FilterCollection::builder()
        .add_response_generation(error_handler)
        .add_response_generation(redirect_handler)
        .build();

    let services = collection.build_services()?;

    // Simulate error scenario
    let request = crate::filter::utils::test_utils::create_test_request();
    let response = services.response_generation.oneshot(request).await?;

    println!("Error handling example completed");
    Ok(())
}

/// Example: Performance-optimized filter chain using Tower layers directly
pub async fn performance_optimized_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create filters
    let access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![IpRef::Net("0.0.0.0/0".parse()?)]) // Allow all for performance testing
        .build();
    let access_handler = AccessControlFilterHandler::try_from(access_config)?;

    let mut headers = HeaderMap::new();
    headers.insert("x-cache-control", "no-cache".parse()?);

    let header_config = HeaderModifierFilter::builder().add(headers).build();
    let header_handler = HeaderModifierFilterHandler::try_from(header_config)?;

    // Use Tower's ServiceBuilder for optimal composition
    let base_service = crate::filter::utils::test_utils::create_mock_service();

    let optimized_service = ServiceBuilder::new()
        .layer(InboundRequestLayer::new(access_handler))
        .layer(PreBackendLayer::new(header_handler))
        .service(base_service);

    // Use the optimized service
    let request = crate::filter::utils::test_utils::create_test_request();
    let response = optimized_service.oneshot(request).await?;

    println!("Performance-optimized example completed");
    Ok(())
}

/// Example: Testing filter composition
#[cfg(test)]
pub async fn test_filter_composition() -> Result<(), Box<dyn std::error::Error>> {
    use crate::filter::utils::test_utils::{assert_filter_status, assert_header_exists};

    // Create a test filter chain
    let access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![IpRef::Addr("192.168.1.1".parse()?)])
        .build();
    let access_handler = AccessControlFilterHandler::try_from(access_config)?;

    let mut headers = HeaderMap::new();
    headers.insert("x-test-header", "test-value".parse()?);

    let header_config = HeaderModifierFilter::builder().add(headers).build();
    let header_handler = HeaderModifierFilterHandler::try_from(header_config)?;

    let collection = FilterCollection::builder()
        .add_inbound_request(access_handler)
        .add_pre_backend(header_handler)
        .build();

    let services = collection.build_services()?;

    // Test the composition
    let request = crate::filter::utils::test_utils::create_test_request();
    let response = services.inbound_request.oneshot(request).await?;

    // Verify the response
    assert_eq!(response.status(), StatusCode::OK);

    // Test pre-backend stage
    let request2 = crate::filter::utils::test_utils::create_test_request();
    let response2 = services.pre_backend.oneshot(request2).await?;
    assert_header_exists(&response2, "x-test-header");

    println!("Filter composition test passed");
    Ok(())
}
