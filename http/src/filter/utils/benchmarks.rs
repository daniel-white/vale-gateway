//! Benchmarking utilities for filter performance testing
//!
//! This module provides benchmarking utilities to measure filter performance
//! and identify optimization opportunities.

use crate::filter::{FilterCollection, FilterRequest, FilterResponse, FilterServiceFactory};
use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use std::time::Duration;
use tower::ServiceExt;

/// Benchmark a single filter handler
pub fn bench_single_filter<F>(c: &mut Criterion, name: &str, mut filter: F, request: FilterRequest)
where
    F: tower::Service<FilterRequest, Response = FilterResponse> + Clone + Send + 'static,
    F::Error: std::fmt::Debug,
    F::Future: Send,
{
    c.bench_function(name, |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let req = black_box(request.clone());
                let result = filter.clone().oneshot(req).await;
                black_box(result.unwrap())
            })
    });
}

/// Benchmark a filter chain composition
pub fn bench_filter_chain(
    c: &mut Criterion,
    name: &str,
    collection: FilterCollection,
    request: FilterRequest,
) {
    let services = collection.build_services().unwrap();

    c.bench_function(name, |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let req = black_box(request.clone());
                let result = services.inbound_request.clone().oneshot(req).await;
                black_box(result.unwrap())
            })
    });
}

/// Benchmark filter throughput with varying request loads
pub fn bench_filter_throughput<F>(
    c: &mut Criterion,
    name: &str,
    create_filter: F,
    request_counts: &[usize],
) where
    F: Fn() -> Box<
            dyn tower::Service<
                    FilterRequest,
                    Response = FilterResponse,
                    Error = Box<dyn std::error::Error + Send + Sync>,
                > + Send
                + Sync,
        > + Clone,
{
    let mut group = c.benchmark_group(format!("{}_throughput", name));

    for &count in request_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("requests", count), &count, |b, &count| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let filter = create_filter();
                    let mut handles = Vec::new();

                    for _ in 0..count {
                        let mut service = filter.clone();
                        let request = create_benchmark_request();
                        let handle = tokio::spawn(async move { service.oneshot(request).await });
                        handles.push(handle);
                    }

                    for handle in handles {
                        black_box(handle.await.unwrap().unwrap());
                    }
                });
        });
    }
    group.finish();
}

/// Benchmark filter latency under different conditions
pub fn bench_filter_latency<F>(
    c: &mut Criterion,
    name: &str,
    filter: F,
    scenarios: &[(&str, FilterRequest)],
) where
    F: tower::Service<FilterRequest, Response = FilterResponse> + Clone + Send + 'static,
    F::Error: std::fmt::Debug,
    F::Future: Send,
{
    let mut group = c.benchmark_group(format!("{}_latency", name));

    for (scenario_name, request) in scenarios {
        group.bench_function(*scenario_name, |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let req = black_box(request.clone());
                    let result = filter.clone().oneshot(req).await;
                    black_box(result.unwrap())
                });
        });
    }
    group.finish();
}

/// Benchmark service factory performance
pub fn bench_service_factory(c: &mut Criterion) {
    use crate::filter::handlers::AccessControlFilterHandler;
    use vg_config::http::filter::access_control::{AccessControlEffect, AccessControlFilter};
    use vg_core::net::IpRef;

    let mut group = c.benchmark_group("service_factory");

    // Benchmark creating services with different numbers of filters
    let filter_counts = [1, 5, 10, 20];

    for &count in &filter_counts {
        group.bench_function(BenchmarkId::new("create_service", count), |b| {
            b.iter(|| {
                let mut filters = Vec::new();
                for i in 0..count {
                    let config = AccessControlFilter::builder()
                        .effect(AccessControlEffect::Allow)
                        .clients(vec![IpRef::Addr(
                            format!("192.168.1.{}", i + 1).parse().unwrap(),
                        )])
                        .build();
                    let handler = AccessControlFilterHandler::try_from(config).unwrap();
                    filters.push(handler);
                }

                let service = FilterServiceFactory::create_inbound_service(filters);
                black_box(service)
            });
        });
    }

    group.finish();
}

/// Benchmark filter collection building
pub fn bench_filter_collection_building(c: &mut Criterion) {
    use crate::filter::handlers::{AccessControlFilterHandler, HeaderModifierFilterHandler};
    use http::HeaderMap;
    use vg_config::http::filter::{
        access_control::{AccessControlEffect, AccessControlFilter},
        header_modifier::HeaderModifierFilter,
    };
    use vg_core::net::IpRef;

    let mut group = c.benchmark_group("collection_building");

    // Benchmark building collections with different complexities
    group.bench_function("simple_collection", |b| {
        b.iter(|| {
            let access_config = AccessControlFilter::builder()
                .effect(AccessControlEffect::Allow)
                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                .build();
            let access_handler = AccessControlFilterHandler::try_from(access_config).unwrap();

            let collection = FilterCollection::builder()
                .add_inbound_request(access_handler)
                .build();

            black_box(collection)
        });
    });

    group.bench_function("complex_collection", |b| {
        b.iter(|| {
            // Create multiple filters
            let access_config = AccessControlFilter::builder()
                .effect(AccessControlEffect::Allow)
                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                .build();
            let access_handler = AccessControlFilterHandler::try_from(access_config).unwrap();

            let mut headers = HeaderMap::new();
            headers.insert("x-benchmark", "test".parse().unwrap());
            let header_config = HeaderModifierFilter::builder().add(headers).build();
            let header_handler = HeaderModifierFilterHandler::try_from(header_config).unwrap();

            let collection = FilterCollection::builder()
                .add_inbound_request(access_handler)
                .add_pre_backend(header_handler)
                .build();

            black_box(collection)
        });
    });

    group.finish();
}

/// Benchmark concurrent filter execution
pub fn bench_concurrent_execution<F>(
    c: &mut Criterion,
    name: &str,
    create_service: F,
    concurrency_levels: &[usize],
) where
    F: Fn() -> Box<
            dyn tower::Service<
                    FilterRequest,
                    Response = FilterResponse,
                    Error = Box<dyn std::error::Error + Send + Sync>,
                > + Send
                + Sync,
        > + Clone,
{
    let mut group = c.benchmark_group(format!("{}_concurrent", name));

    for &concurrency in concurrency_levels {
        group.bench_function(BenchmarkId::new("concurrent_requests", concurrency), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let service = create_service();
                    let mut handles = Vec::new();

                    for _ in 0..concurrency {
                        let mut svc = service.clone();
                        let request = create_benchmark_request();
                        let handle = tokio::spawn(async move { svc.oneshot(request).await });
                        handles.push(handle);
                    }

                    for handle in handles {
                        black_box(handle.await.unwrap().unwrap());
                    }
                });
        });
    }

    group.finish();
}

/// Create a standard benchmark request for consistent testing
pub fn create_benchmark_request() -> FilterRequest {
    use crate::filter::utils::test_utils::create_test_request;
    create_test_request()
}

/// Create benchmark requests with different characteristics
pub fn create_benchmark_scenarios() -> Vec<(&'static str, FilterRequest)> {
    use crate::filter::utils::test_utils::{
        create_test_request, create_test_request_with_headers, create_test_request_with_ip,
    };

    vec![
        ("simple_request", create_test_request()),
        (
            "request_with_headers",
            create_test_request_with_headers(&[
                ("x-test-header", "test-value"),
                ("user-agent", "benchmark-client/1.0"),
                ("accept", "application/json"),
            ]),
        ),
        (
            "request_with_many_headers",
            create_test_request_with_headers(&[
                ("header-1", "value-1"),
                ("header-2", "value-2"),
                ("header-3", "value-3"),
                ("header-4", "value-4"),
                ("header-5", "value-5"),
                ("header-6", "value-6"),
                ("header-7", "value-7"),
                ("header-8", "value-8"),
                ("header-9", "value-9"),
                ("header-10", "value-10"),
            ]),
        ),
        (
            "ipv4_request",
            create_test_request_with_ip("192.168.1.100".parse().unwrap()),
        ),
        (
            "ipv6_request",
            create_test_request_with_ip("2001:db8::1".parse().unwrap()),
        ),
    ]
}

/// Benchmark memory allocation patterns
pub fn bench_memory_usage<F>(c: &mut Criterion, name: &str, create_filter: F)
where
    F: Fn() -> Box<
        dyn tower::Service<
                FilterRequest,
                Response = FilterResponse,
                Error = Box<dyn std::error::Error + Send + Sync>,
            > + Send
            + Sync,
    >,
{
    c.bench_function(&format!("{}_memory", name), |b| {
        b.iter(|| {
            let filter = black_box(create_filter());
            std::mem::drop(filter);
        })
    });
}

/// Benchmark filter chain composition performance
pub fn bench_chain_composition(c: &mut Criterion) {
    use crate::filter::handlers::{AccessControlFilterHandler, HeaderModifierFilterHandler};
    use http::HeaderMap;
    use vg_config::http::filter::{
        access_control::{AccessControlEffect, AccessControlFilter},
        header_modifier::HeaderModifierFilter,
    };
    use vg_core::net::IpRef;

    let mut group = c.benchmark_group("chain_composition");

    // Benchmark different chain lengths
    let chain_lengths = [1, 3, 5, 10];

    for &length in &chain_lengths {
        group.bench_function(BenchmarkId::new("chain_length", length), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let mut builder = FilterCollection::builder();

                    // Add filters to create a chain of specified length
                    for i in 0..length {
                        if i % 2 == 0 {
                            // Add access control filter
                            let config = AccessControlFilter::builder()
                                .effect(AccessControlEffect::Allow)
                                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                                .build();
                            let handler = AccessControlFilterHandler::try_from(config).unwrap();
                            builder = builder.add_inbound_request(handler);
                        } else {
                            // Add header modifier filter
                            let mut headers = HeaderMap::new();
                            headers.insert(
                                &format!("x-filter-{}", i),
                                format!("value-{}", i).parse().unwrap(),
                            );
                            let config = HeaderModifierFilter::builder().add(headers).build();
                            let handler = HeaderModifierFilterHandler::try_from(config).unwrap();
                            builder = builder.add_pre_backend(handler);
                        }
                    }

                    let collection = builder.build();
                    let services = collection.build_services().unwrap();

                    // Execute a request through the chain
                    let request = create_benchmark_request();
                    let result = services.inbound_request.oneshot(request).await;
                    black_box(result.unwrap())
                });
        });
    }

    group.finish();
}

/// Run all performance benchmarks
pub fn run_all_benchmarks(c: &mut Criterion) {
    bench_service_factory(c);
    bench_filter_collection_building(c);
    bench_chain_composition(c);

    // Add more comprehensive benchmarks
    bench_filter_throughput(
        c,
        "access_control",
        || {
            use crate::filter::handlers::AccessControlFilterHandler;
            use vg_config::http::filter::access_control::{
                AccessControlEffect, AccessControlFilter,
            };
            use vg_core::net::IpRef;

            let config = AccessControlFilter::builder()
                .effect(AccessControlEffect::Allow)
                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                .build();
            let handler = AccessControlFilterHandler::try_from(config).unwrap();
            Box::new(FilterServiceFactory::create_single_filter_service(handler))
        },
        &[1, 10, 50, 100],
    );

    bench_concurrent_execution(
        c,
        "header_modifier",
        || {
            use crate::filter::handlers::HeaderModifierFilterHandler;
            use http::HeaderMap;
            use vg_config::http::filter::header_modifier::HeaderModifierFilter;

            let mut headers = HeaderMap::new();
            headers.insert("x-concurrent-test", "value".parse().unwrap());
            let config = HeaderModifierFilter::builder().add(headers).build();
            let handler = HeaderModifierFilterHandler::try_from(config).unwrap();
            Box::new(FilterServiceFactory::create_single_filter_service(handler))
        },
        &[1, 5, 10, 20],
    );
}
