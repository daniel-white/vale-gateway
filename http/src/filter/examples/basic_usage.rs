//! Basic usage examples for the filter system
//!
//! This module demonstrates simple, common use cases for HTTP filters.

#![allow(unused)]

use crate::filter::{
    FilterCollection, FilterServiceFactory,
    handlers::{AccessControlFilterHandler, HeaderModifierFilterHandler},
};
use http::HeaderMap;
use std::collections::HashSet;
use tower::ServiceExt;
use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
use vg_config::http::filter::header_modifier::HeaderModifierFilter;
use vg_core::net::IpRef;

/// Example: Creating a simple access control filter
pub async fn simple_access_control_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create access control configuration
    let config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![
            IpRef::Addr("192.168.1.1".parse()?),
            IpRef::Net("10.0.0.0/8".parse()?),
        ])
        .build();

    // Create filter handler from configuration
    let handler = AccessControlFilterHandler::try_from(config)?;

    // Use the filter
    let request = crate::filter::utils::test_utils::create_test_request();
    let response = handler.oneshot(request).await?;

    println!("Access control filter applied successfully");
    Ok(())
}

/// Example: Creating a header modifier filter
pub async fn simple_header_modifier_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create header modification configuration
    let mut add_headers = HeaderMap::new();
    add_headers.insert("x-custom-header", "custom-value".parse()?);

    let mut remove_headers = HashSet::new();
    remove_headers.insert("x-unwanted-header".parse()?);

    let config = HeaderModifierFilter::builder()
        .add(add_headers)
        .remove(remove_headers)
        .build();

    // Create filter handler from configuration
    let handler = HeaderModifierFilterHandler::try_from(config)?;

    // Use the filter
    let request = crate::filter::utils::test_utils::create_test_request_with_headers(&[
        ("x-unwanted-header", "should-be-removed"),
        ("x-existing-header", "should-remain"),
    ]);

    let response = handler.oneshot(request).await?;

    println!("Header modifier filter applied successfully");
    Ok(())
}

/// Example: Creating a filter collection with multiple filters
pub async fn simple_filter_collection_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create access control filter
    let access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![IpRef::Addr("192.168.1.1".parse()?)])
        .build();
    let access_handler = AccessControlFilterHandler::try_from(access_config)?;

    // Create header modifier filter
    let mut add_headers = HeaderMap::new();
    add_headers.insert("x-processed-by", "vale-gateway".parse()?);

    let header_config = HeaderModifierFilter::builder().add(add_headers).build();
    let header_handler = HeaderModifierFilterHandler::try_from(header_config)?;

    // Build filter collection
    let collection = FilterCollection::builder()
        .add_inbound_request(access_handler)
        .add_pre_backend(header_handler)
        .build();

    // Create services from collection
    let services = collection.build_services()?;

    // Use the composed service
    let request = crate::filter::utils::test_utils::create_test_request();
    let response = services.inbound_request.oneshot(request).await?;

    println!("Filter collection applied successfully");
    Ok(())
}

/// Example: Using the service factory directly
pub async fn service_factory_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create individual filters
    let access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![IpRef::Addr("192.168.1.1".parse()?)])
        .build();
    let access_handler = AccessControlFilterHandler::try_from(access_config)?;

    // Use service factory to create composed service
    let service = FilterServiceFactory::create_inbound_service(vec![Box::new(access_handler)]);

    // Use the service
    let request = crate::filter::utils::test_utils::create_test_request();
    let response = service.oneshot(request).await?;

    println!("Service factory example completed successfully");
    Ok(())
}
