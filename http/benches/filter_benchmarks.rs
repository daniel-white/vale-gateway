//! Comprehensive benchmarks for the HTTP filter system
//!
//! This benchmark suite tests the performance characteristics of the filter system
//! including individual filter performance, composition overhead, and memory usage.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use http::{HeaderMap, StatusCode};
use std::hint::black_box;
use std::str::FromStr;
use tower::ServiceExt;
use vg_config::http::filter::{
    access_control::{AccessControlEffect, AccessControlFilter},
    backend_uri_rewriter::BackendUriRewriterFilter,
    header_modifier::HeaderModifierFilter,
    redirect_response::RedirectResponseFilter,
    static_response::{Body, BodyContent, StaticResponseFilter},
};
use vg_config::http::rewriting::uri::{PathRewrite, UriRewriter};
use vg_core::{http::content_type::ContentTypeBuf, net::IpRef};
use vg_http::filter::{
    FilterCollection, FilterServiceFactory,
    handlers::{
        AccessControlFilterHandler, BackendUriRewriterFilterHandler, HeaderModifierFilterHandler,
        RedirectResponseFilterHandler, StaticResponseFilterHandler,
    },
    utils::test_utils::create_test_request,
};

/// Benchmark individual filter handlers
fn bench_individual_filters(c: &mut Criterion) {
    let mut group = c.benchmark_group("individual_filters");

    // Benchmark AccessControlFilterHandler
    let access_config = AccessControlFilter::builder()
        .effect(AccessControlEffect::Allow)
        .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
        .build();
    let access_handler = AccessControlFilterHandler::try_from(access_config).unwrap();
    let request = create_test_request();

    group.bench_function("access_control", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = access_handler.clone().oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    // Benchmark HeaderModifierFilterHandler
    let mut headers = HeaderMap::new();
    headers.insert("x-benchmark", "test-value".parse().unwrap());
    headers.insert("x-timestamp", "2024-01-01T00:00:00Z".parse().unwrap());

    let header_config = HeaderModifierFilter::builder()
        .add(headers)
        .set(HeaderMap::new())
        .remove(Vec::new())
        .build();
    let header_handler = HeaderModifierFilterHandler::try_from(header_config).unwrap();

    group.bench_function("header_modifier", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = header_handler.clone().oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    // Benchmark BackendUriRewriterFilterHandler
    let rewriter = UriRewriter::builder()
        .scheme(http::uri::Scheme::HTTP)
        .host("localhost".to_string())
        .port(vg_core::net::Port::try_from(8080u16).unwrap())
        .path(PathRewrite::ReplacePrefixWith("/api/v1".to_string()))
        .build();
    let uri_config = BackendUriRewriterFilter::builder().uri(rewriter).build();
    let uri_handler = BackendUriRewriterFilterHandler::try_from(uri_config).unwrap();

    group.bench_function("backend_uri_rewriter", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = uri_handler.clone().oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    // Benchmark RedirectResponseFilterHandler
    let redirect_config = RedirectResponseFilter::builder()
        .status_code(StatusCode::TEMPORARY_REDIRECT)
        .uri(
            UriRewriter::builder()
                .scheme(http::uri::Scheme::HTTPS)
                .host("example.com".to_string())
                .port(vg_core::net::Port::HTTPS)
                .path(PathRewrite::ReplaceWith("/redirect".to_string()))
                .build(),
        )
        .build();
    let redirect_handler = RedirectResponseFilterHandler::try_from(&redirect_config).unwrap();

    group.bench_function("redirect_response", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = redirect_handler.clone().oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    // Benchmark StaticResponseFilterHandler
    let static_config = StaticResponseFilter::builder()
        .status_code(StatusCode::OK)
        .body(
            Body::builder()
                .content_type(ContentTypeBuf::from_str("text/plain").unwrap())
                .content(BodyContent::Text("Benchmark response body".to_string()))
                .build(),
        )
        .build();
    let static_handler = StaticResponseFilterHandler::try_from(static_config).unwrap();

    group.bench_function("static_response", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = static_handler.clone().oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    group.finish();
}

/// Benchmark filter composition overhead
fn bench_filter_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_composition");
    let request = create_test_request();

    group.bench_function("single_filter", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                // Create a new collection and services for each iteration to avoid cloning issues
                let collection = FilterCollection::builder()
                    .add_inbound_request(
                        AccessControlFilterHandler::try_from(
                            AccessControlFilter::builder()
                                .effect(AccessControlEffect::Allow)
                                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                                .build(),
                        )
                        .unwrap(),
                    )
                    .build();
                let services = collection.build_services().unwrap();
                let result = services.inbound_request.oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    group.bench_function("multi_filter_composition", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                // Create new services for each iteration to avoid cloning issues
                let collection = FilterCollection::builder()
                    .add_inbound_request(
                        AccessControlFilterHandler::try_from(
                            AccessControlFilter::builder()
                                .effect(AccessControlEffect::Allow)
                                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                                .build(),
                        )
                        .unwrap(),
                    )
                    .add_pre_backend(
                        HeaderModifierFilterHandler::try_from(
                            HeaderModifierFilter::builder()
                                .add({
                                    let mut headers = HeaderMap::new();
                                    headers.insert("x-multi-filter", "composed".parse().unwrap());
                                    headers
                                })
                                .set(HeaderMap::new())
                                .remove(Vec::new())
                                .build(),
                        )
                        .unwrap(),
                    )
                    .add_backend_request(
                        BackendUriRewriterFilterHandler::try_from(
                            BackendUriRewriterFilter::builder()
                                .uri(
                                    UriRewriter::builder()
                                        .scheme(http::uri::Scheme::HTTP)
                                        .host("localhost".to_string())
                                        .port(vg_core::net::Port::try_from(8080u16).unwrap())
                                        .path(PathRewrite::ReplaceWith("/".to_string()))
                                        .build(),
                                )
                                .build(),
                        )
                        .unwrap(),
                    )
                    .build();
                let services = collection.build_services().unwrap();

                // Test each stage
                let inbound_result = services
                    .inbound_request
                    .oneshot(request.clone())
                    .await
                    .unwrap();
                let pre_backend_result =
                    services.pre_backend.oneshot(request.clone()).await.unwrap();
                let backend_result = services
                    .backend_request
                    .oneshot(request.clone())
                    .await
                    .unwrap();

                black_box((inbound_result, pre_backend_result, backend_result))
            })
        })
    });

    group.finish();
}

/// Benchmark service factory performance
fn bench_service_factory_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("service_factory");

    // Benchmark service creation with different numbers of filters
    let filter_counts = [1, 5, 10, 20, 50];

    for &count in &filter_counts {
        group.bench_function(BenchmarkId::new("create_inbound_service", count), |b| {
            b.iter(|| {
                let mut filters = Vec::new();
                for i in 0..count {
                    let config = AccessControlFilter::builder()
                        .effect(AccessControlEffect::Allow)
                        .clients(vec![IpRef::Addr(
                            format!("192.168.1.{}", (i % 254) + 1).parse().unwrap(),
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

/// Benchmark throughput under load
fn bench_throughput_under_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_under_load");

    let request_counts = [1, 10, 50, 100, 500];

    for &count in &request_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_requests", count),
            &count,
            |b, &count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();

                        for _ in 0..count {
                            // Create a new service for each request to avoid cloning issues
                            let collection = FilterCollection::builder()
                                .add_inbound_request(
                                    AccessControlFilterHandler::try_from(
                                        AccessControlFilter::builder()
                                            .effect(AccessControlEffect::Allow)
                                            .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                                            .build(),
                                    )
                                    .unwrap(),
                                )
                                .build();
                            let service = collection.build_services().unwrap().inbound_request;
                            let request = create_test_request();
                            let handle =
                                tokio::spawn(async move { service.oneshot(request).await });
                            handles.push(handle);
                        }

                        for handle in handles {
                            black_box(handle.await.unwrap().unwrap());
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    // Benchmark filter creation and destruction
    group.bench_function("filter_creation_destruction", |b| {
        b.iter(|| {
            let config = AccessControlFilter::builder()
                .effect(AccessControlEffect::Allow)
                .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                .build();
            let handler = AccessControlFilterHandler::try_from(config).unwrap();
            let service = FilterServiceFactory::create_single_filter_service(handler);
            black_box(service);
            // Service is dropped here
        });
    });

    // Benchmark collection building
    group.bench_function("collection_building", |b| {
        b.iter(|| {
            let collection = FilterCollection::builder()
                .add_inbound_request(
                    AccessControlFilterHandler::try_from(
                        AccessControlFilter::builder()
                            .effect(AccessControlEffect::Allow)
                            .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                            .build(),
                    )
                    .unwrap(),
                )
                .add_pre_backend(
                    HeaderModifierFilterHandler::try_from(
                        HeaderModifierFilter::builder()
                            .add({
                                let mut headers = HeaderMap::new();
                                headers.insert("x-memory-test", "allocation".parse().unwrap());
                                headers
                            })
                            .set(HeaderMap::new())
                            .remove(Vec::new())
                            .build(),
                    )
                    .unwrap(),
                )
                .build();

            let services = collection.build_services().unwrap();
            black_box(services);
            // Collection and services are dropped here
        });
    });

    group.finish();
}

/// Benchmark different request scenarios
fn bench_request_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_scenarios");

    // Test different request types
    let scenarios = [
        ("simple_request", create_test_request()),
        ("request_with_headers", {
            use vg_http::filter::utils::test_utils::create_test_request_with_headers;
            create_test_request_with_headers(&[
                ("user-agent", "benchmark-client/1.0"),
                ("accept", "application/json"),
                ("x-remove-me", "should-be-removed"),
            ])
        }),
        ("request_with_many_headers", {
            use vg_http::filter::utils::test_utils::create_test_request_with_headers;
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
                ("x-remove-me", "should-be-removed"),
            ])
        }),
    ];

    for (scenario_name, request) in &scenarios {
        group.bench_function(*scenario_name, |b| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    // Create new services for each iteration
                    let collection = FilterCollection::builder()
                        .add_inbound_request(
                            AccessControlFilterHandler::try_from(
                                AccessControlFilter::builder()
                                    .effect(AccessControlEffect::Allow)
                                    .clients(vec![IpRef::Net("0.0.0.0/0".parse().unwrap())])
                                    .build(),
                            )
                            .unwrap(),
                        )
                        .add_pre_backend(
                            HeaderModifierFilterHandler::try_from(
                                HeaderModifierFilter::builder()
                                    .add({
                                        let mut headers = HeaderMap::new();
                                        headers
                                            .insert("x-scenario-test", "active".parse().unwrap());
                                        headers
                                    })
                                    .set(HeaderMap::new())
                                    .remove(vec!["x-remove-me".parse().unwrap()])
                                    .build(),
                            )
                            .unwrap(),
                        )
                        .build();
                    let services = collection.build_services().unwrap();

                    let inbound_result = services
                        .inbound_request
                        .oneshot(request.clone())
                        .await
                        .unwrap();
                    let pre_backend_result =
                        services.pre_backend.oneshot(request.clone()).await.unwrap();
                    black_box((inbound_result, pre_backend_result))
                })
            });
        });
    }

    group.finish();
}

/// Benchmark error handling performance
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    // Test access control denial (returns 403, not error)
    let deny_handler = AccessControlFilterHandler::try_from(
        AccessControlFilter::builder()
            .effect(AccessControlEffect::Deny)
            .clients(vec![IpRef::Addr("192.168.1.1".parse().unwrap())])
            .build(),
    )
    .unwrap();

    let request = {
        use vg_http::filter::utils::test_utils::create_test_request_with_ip;
        create_test_request_with_ip("192.168.1.1".parse().unwrap())
    };

    group.bench_function("access_denied_response", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = deny_handler.clone().oneshot(request.clone()).await;
                black_box(result.unwrap())
            })
        })
    });

    // Test static error response
    let error_handler = StaticResponseFilterHandler::try_from(
        StaticResponseFilter::builder()
            .status_code(StatusCode::SERVICE_UNAVAILABLE)
            .body(
                Body::builder()
                    .content_type(ContentTypeBuf::from_str("text/plain").unwrap())
                    .content(BodyContent::Text(
                        "Service temporarily unavailable".to_string(),
                    ))
                    .build(),
            )
            .build(),
    )
    .unwrap();

    group.bench_function("static_error_response", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.iter(|| {
            rt.block_on(async {
                let result = error_handler.clone().oneshot(create_test_request()).await;
                black_box(result.unwrap())
            })
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_individual_filters,
    bench_filter_composition,
    bench_service_factory_performance,
    bench_throughput_under_load,
    bench_memory_allocation,
    bench_request_scenarios,
    bench_error_handling
);

criterion_main!(benches);
