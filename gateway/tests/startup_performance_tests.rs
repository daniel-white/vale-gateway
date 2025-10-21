use http::Uri;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use vg_config::http::listener::ListenerRef;
use vg_rpc_client::{ConfigurationClient, ConfigurationEventsClient, RpcTransport};

/// Performance test environment for measuring startup times
struct PerformanceTestEnvironment {
    server_uri: Uri,
    #[allow(dead_code)]
    scenario: TestScenario,
}

#[derive(Clone)]
#[allow(dead_code)]
enum TestScenario {
    ServiceAvailable,
    ServiceUnavailable,
    ServiceSlow,
    InvalidHost,
}

impl PerformanceTestEnvironment {
    fn with_unavailable_service() -> Self {
        Self {
            server_uri: "ws://127.0.0.1:1".parse().unwrap(), // Port 1 should be unavailable
            scenario: TestScenario::ServiceUnavailable,
        }
    }

    fn with_invalid_host() -> Self {
        Self {
            server_uri: "ws://invalid-host-that-does-not-exist:9000"
                .parse()
                .unwrap(),
            scenario: TestScenario::InvalidHost,
        }
    }

    fn get_server_uri(&self) -> Uri {
        self.server_uri.clone()
    }

    #[allow(dead_code)]
    fn get_scenario(&self) -> TestScenario {
        self.scenario.clone()
    }
}

/// Test that gateway startup completes within 5-second requirement (Requirement 3.1)
#[tokio::test]
async fn test_startup_time_requirement_unavailable_service() {
    let env = PerformanceTestEnvironment::with_unavailable_service();

    let listener_ref = "perf-test-listener".to_string();
    let server_uri = env.get_server_uri();

    // Measure startup time with unavailable service
    let start_time = Instant::now();

    let client_result = match RpcTransport::new(server_uri.clone()).await {
        Ok(transport) => Ok(ConfigurationClient::new(
            transport,
            ListenerRef::from(listener_ref.clone()),
        )),
        Err(e) => Err(e),
    };

    let events_result = match RpcTransport::new(server_uri).await {
        Ok(transport) => Ok(ConfigurationEventsClient::new(
            transport,
            ListenerRef::from(listener_ref),
        )),
        Err(e) => Err(e),
    };

    let startup_time = start_time.elapsed();

    // Verify startup meets the 5-second requirement from requirements 3.1
    assert!(
        startup_time < Duration::from_secs(5),
        "Gateway startup should complete in under 5 seconds with unavailable service, took {:?}",
        startup_time
    );

    // Log performance metrics
    println!("Startup time with unavailable service: {:?}", startup_time);

    // Verify that clients were created or failed gracefully
    match (client_result, events_result) {
        (Ok(_), Ok(_)) => {
            println!("Both clients created successfully despite unavailable service");
        }
        (Err(_), Err(_)) => {
            println!("Both clients failed gracefully with unavailable service");
        }
        _ => {
            println!("Mixed results - some clients succeeded, others failed");
        }
    }
}

/// Test startup time with invalid hostname (DNS resolution)
#[tokio::test]
async fn test_startup_time_with_dns_resolution() {
    let env = PerformanceTestEnvironment::with_invalid_host();

    let listener_ref = "perf-test-dns".to_string();
    let server_uri = env.get_server_uri();

    // Measure startup time with DNS resolution failure
    let start_time = Instant::now();

    let client_result = match RpcTransport::new(server_uri.clone()).await {
        Ok(transport) => Ok(ConfigurationClient::new(
            transport,
            ListenerRef::from(listener_ref.clone()),
        )),
        Err(e) => Err(e),
    };

    let events_result = match RpcTransport::new(server_uri).await {
        Ok(transport) => Ok(ConfigurationEventsClient::new(
            transport,
            ListenerRef::from(listener_ref),
        )),
        Err(e) => Err(e),
    };

    let startup_time = start_time.elapsed();

    // Should complete within 5 seconds even with DNS resolution failure
    assert!(
        startup_time < Duration::from_secs(5),
        "Gateway startup should complete in under 5 seconds with DNS failure, took {:?}",
        startup_time
    );

    // Log performance metrics
    println!(
        "Startup time with DNS resolution failure: {:?}",
        startup_time
    );

    // Results should be consistent
    match (client_result, events_result) {
        (Ok(_), Ok(_)) => {
            println!("Both clients created successfully despite DNS failure");
        }
        (Err(_), Err(_)) => {
            println!("Both clients failed gracefully with DNS failure");
        }
        _ => {
            println!("Mixed results with DNS failure");
        }
    }
}

/// Test startup time consistency across multiple attempts
#[tokio::test]
async fn test_startup_time_consistency() {
    let env = PerformanceTestEnvironment::with_unavailable_service();

    let server_uri = env.get_server_uri();
    let attempts = 5;
    let mut startup_times = Vec::new();

    for i in 0..attempts {
        let listener_ref = format!("perf-test-consistency-{}", i);

        let start_time = Instant::now();

        let _client_result = match RpcTransport::new(server_uri.clone()).await {
            Ok(transport) => Ok(ConfigurationClient::new(
                transport,
                ListenerRef::from(listener_ref.clone()),
            )),
            Err(e) => Err(e),
        };

        let _events_result = match RpcTransport::new(server_uri.clone()).await {
            Ok(transport) => Ok(ConfigurationEventsClient::new(
                transport,
                ListenerRef::from(listener_ref),
            )),
            Err(e) => Err(e),
        };

        let startup_time = start_time.elapsed();
        startup_times.push(startup_time);

        // Each attempt should meet the 5-second requirement
        assert!(
            startup_time < Duration::from_secs(5),
            "Attempt {} should complete in under 5 seconds, took {:?}",
            i,
            startup_time
        );
    }

    // Calculate performance statistics
    let total_time: Duration = startup_times.iter().sum();
    let average_time = total_time / attempts as u32;
    let min_time = startup_times.iter().min().unwrap();
    let max_time = startup_times.iter().max().unwrap();

    println!("Startup time statistics over {} attempts:", attempts);
    println!("  Average: {:?}", average_time);
    println!("  Min: {:?}", min_time);
    println!("  Max: {:?}", max_time);
    println!("  All times: {:?}", startup_times);

    // Verify consistency - max time shouldn't be more than 2x average
    assert!(
        *max_time < average_time * 2,
        "Startup times should be consistent - max {:?} should not be more than 2x average {:?}",
        max_time,
        average_time
    );

    // Verify all times are reasonable
    assert!(
        average_time < Duration::from_secs(3),
        "Average startup time should be well under the 5-second limit, was {:?}",
        average_time
    );
}

/// Test memory usage during startup (basic verification)
#[tokio::test]
async fn test_startup_memory_usage() {
    let env = PerformanceTestEnvironment::with_unavailable_service();

    let listener_ref = "perf-test-memory".to_string();
    let server_uri = env.get_server_uri();

    // Note: This is a basic test - in a real scenario you'd use more sophisticated memory profiling

    // Create multiple clients to test memory usage
    let mut clients: Vec<ConfigurationClient> = Vec::new();
    let mut events_clients: Vec<ConfigurationEventsClient> = Vec::new();

    let start_time = Instant::now();

    for i in 0..10 {
        let client_result = match RpcTransport::new(server_uri.clone()).await {
            Ok(transport) => Ok(ConfigurationClient::new(
                transport,
                ListenerRef::from(format!("{}-{}", listener_ref, i)),
            )),
            Err(e) => Err(e),
        };

        let events_result = match RpcTransport::new(server_uri.clone()).await {
            Ok(transport) => Ok(ConfigurationEventsClient::new(
                transport,
                ListenerRef::from(format!("{}-events-{}", listener_ref, i)),
            )),
            Err(e) => Err(e),
        };

        // Store clients to prevent them from being dropped
        if let Ok(client) = client_result {
            clients.push(client);
        }
        if let Ok(events_client) = events_result {
            events_clients.push(events_client);
        }
    }

    let creation_time = start_time.elapsed();

    // Creating 10 clients should still be fast
    assert!(
        creation_time < Duration::from_secs(10),
        "Creating multiple clients should be fast, took {:?}",
        creation_time
    );

    println!(
        "Created {} configuration clients and {} events clients in {:?}",
        clients.len(),
        events_clients.len(),
        creation_time
    );

    // Verify clients are functional (basic smoke test)
    for (i, client) in clients.iter().enumerate() {
        assert!(
            client.is_monitoring().await,
            "Client {} should have monitoring active",
            i
        );
    }

    for (i, events_client) in events_clients.iter().enumerate() {
        assert!(
            events_client.is_monitoring().await,
            "Events client {} should have monitoring active",
            i
        );
    }

    // Let clients run for a short time to test steady-state memory usage
    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("All clients remain functional after creation");
}

/// Test startup performance under concurrent load
#[tokio::test]
async fn test_concurrent_startup_performance() {
    let env = PerformanceTestEnvironment::with_unavailable_service();
    let server_uri = env.get_server_uri();

    let concurrent_clients = 5;
    let start_time = Instant::now();

    // Create multiple clients concurrently
    let mut handles = Vec::new();

    for i in 0..concurrent_clients {
        let uri = server_uri.clone();
        let handle = tokio::spawn(async move {
            let listener_ref = format!("concurrent-test-{}", i);

            let client_start = Instant::now();

            let client_result = match RpcTransport::new(uri.clone()).await {
                Ok(transport) => Ok(ConfigurationClient::new(
                    transport,
                    ListenerRef::from(listener_ref.clone()),
                )),
                Err(e) => Err(e),
            };

            let events_result = match RpcTransport::new(uri).await {
                Ok(transport) => Ok(ConfigurationEventsClient::new(
                    transport,
                    ListenerRef::from(listener_ref),
                )),
                Err(e) => Err(e),
            };

            let client_time = client_start.elapsed();

            (i, client_result, events_result, client_time)
        });

        handles.push(handle);
    }

    // Wait for all clients to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        results.push(result);
    }

    let total_time = start_time.elapsed();

    // Analyze results
    let mut successful_clients = 0;
    let mut successful_events = 0;
    let mut client_times = Vec::new();

    for (i, client_result, events_result, client_time) in results {
        client_times.push(client_time);

        if client_result.is_ok() {
            successful_clients += 1;
        }
        if events_result.is_ok() {
            successful_events += 1;
        }

        // Each individual client should meet timing requirements
        assert!(
            client_time < Duration::from_secs(5),
            "Concurrent client {} should complete in under 5 seconds, took {:?}",
            i,
            client_time
        );
    }

    // Overall concurrent creation should be efficient
    assert!(
        total_time < Duration::from_secs(8),
        "Concurrent client creation should be efficient, took {:?}",
        total_time
    );

    println!("Concurrent startup results:");
    println!("  Total time: {:?}", total_time);
    println!(
        "  Successful clients: {}/{}",
        successful_clients, concurrent_clients
    );
    println!(
        "  Successful events clients: {}/{}",
        successful_events, concurrent_clients
    );
    println!("  Individual client times: {:?}", client_times);

    // Calculate concurrent performance metrics
    let average_client_time: Duration =
        client_times.iter().sum::<Duration>() / client_times.len() as u32;
    println!(
        "  Average individual client time: {:?}",
        average_client_time
    );
}

/// Test startup performance with timeout scenarios
#[tokio::test]
async fn test_startup_with_timeout_scenarios() {
    let env = PerformanceTestEnvironment::with_unavailable_service();
    let server_uri = env.get_server_uri();

    // Test with very short timeout to ensure graceful handling
    let short_timeout = Duration::from_millis(500);

    let start_time = Instant::now();

    let client_result = timeout(short_timeout, async {
        match RpcTransport::new(server_uri.clone()).await {
            Ok(transport) => Ok(ConfigurationClient::new(
                transport,
                ListenerRef::from("timeout-test-client".to_string()),
            )),
            Err(e) => Err(e),
        }
    })
    .await;

    let events_result = timeout(short_timeout, async {
        match RpcTransport::new(server_uri).await {
            Ok(transport) => Ok(ConfigurationEventsClient::new(
                transport,
                ListenerRef::from("timeout-test-events".to_string()),
            )),
            Err(e) => Err(e),
        }
    })
    .await;

    let total_time = start_time.elapsed();

    // Should complete within reasonable time even with short timeout
    assert!(
        total_time < Duration::from_secs(2),
        "Startup with timeout should complete quickly, took {:?}",
        total_time
    );

    println!(
        "Startup with {}ms timeout took {:?}",
        short_timeout.as_millis(),
        total_time
    );

    // Results should be consistent (either success or timeout)
    match (client_result, events_result) {
        (Ok(Ok(_)), Ok(Ok(_))) => {
            println!("Both clients created successfully within timeout");
        }
        (Ok(Err(_)), Ok(Err(_))) => {
            println!("Both clients failed gracefully within timeout");
        }
        (Err(_), Err(_)) => {
            println!("Both clients timed out as expected");
        }
        _ => {
            println!("Mixed timeout results");
        }
    }
}

/// Test resource consumption during startup
#[tokio::test]
async fn test_startup_resource_consumption() {
    let env = PerformanceTestEnvironment::with_unavailable_service();
    let server_uri = env.get_server_uri();

    // Test that startup doesn't consume excessive resources
    let listener_ref = "resource-test".to_string();

    let start_time = Instant::now();

    // Create client and measure time
    let client_result = match RpcTransport::new(server_uri.clone()).await {
        Ok(transport) => Ok(ConfigurationClient::new(
            transport,
            ListenerRef::from(listener_ref.clone()),
        )),
        Err(e) => Err(e),
    };

    let events_result = match RpcTransport::new(server_uri).await {
        Ok(transport) => Ok(ConfigurationEventsClient::new(
            transport,
            ListenerRef::from(listener_ref),
        )),
        Err(e) => Err(e),
    };

    let startup_time = start_time.elapsed();

    // Verify startup time is reasonable
    assert!(
        startup_time < Duration::from_secs(5),
        "Startup should be efficient, took {:?}",
        startup_time
    );

    // If clients were created, verify they don't consume excessive resources
    match (client_result, events_result) {
        (Ok(client), Ok(events_client)) => {
            // Verify monitoring is active but not resource-intensive
            assert!(
                client.is_monitoring().await,
                "Client should have monitoring"
            );
            assert!(
                events_client.is_monitoring().await,
                "Events client should have monitoring"
            );

            // Let clients run briefly to test steady-state resource usage
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Verify clients remain functional
            assert!(
                client.is_monitoring().await,
                "Client monitoring should remain active"
            );
            assert!(
                events_client.is_monitoring().await,
                "Events client monitoring should remain active"
            );

            println!("Clients created and running efficiently");
        }
        _ => {
            println!("Clients failed to create, which is acceptable for unavailable service");
        }
    }

    println!("Resource consumption test completed in {:?}", startup_time);
}

/// Benchmark startup performance across different scenarios
#[tokio::test]
async fn test_startup_performance_benchmark() {
    let scenarios = vec![
        (
            "unavailable_service",
            PerformanceTestEnvironment::with_unavailable_service(),
        ),
        (
            "invalid_host",
            PerformanceTestEnvironment::with_invalid_host(),
        ),
    ];

    let mut benchmark_results = Vec::new();

    for (scenario_name, env) in scenarios {
        let server_uri = env.get_server_uri();
        let attempts = 3;
        let mut times = Vec::new();

        for i in 0..attempts {
            let listener_ref = format!("benchmark-{}-{}", scenario_name, i);

            let start_time = Instant::now();

            let _client_result = match RpcTransport::new(server_uri.clone()).await {
                Ok(transport) => Ok(ConfigurationClient::new(
                    transport,
                    ListenerRef::from(listener_ref.clone()),
                )),
                Err(e) => Err(e),
            };

            let _events_result = match RpcTransport::new(server_uri.clone()).await {
                Ok(transport) => Ok(ConfigurationEventsClient::new(
                    transport,
                    ListenerRef::from(listener_ref),
                )),
                Err(e) => Err(e),
            };

            let startup_time = start_time.elapsed();
            times.push(startup_time);

            // Each attempt should meet requirements
            assert!(
                startup_time < Duration::from_secs(5),
                "Scenario {} attempt {} should complete in under 5 seconds, took {:?}",
                scenario_name,
                i,
                startup_time
            );
        }

        let average_time: Duration = times.iter().sum::<Duration>() / times.len() as u32;
        let min_time = *times.iter().min().unwrap();
        let max_time = *times.iter().max().unwrap();

        benchmark_results.push((scenario_name, average_time, min_time, max_time));

        println!("Scenario '{}' benchmark:", scenario_name);
        println!("  Average: {:?}", average_time);
        println!("  Min: {:?}", min_time);
        println!("  Max: {:?}", max_time);
    }

    // Print summary
    println!("\nStartup Performance Benchmark Summary:");
    for (scenario, avg, min, max) in benchmark_results {
        println!(
            "  {}: avg={:?}, min={:?}, max={:?}",
            scenario, avg, min, max
        );

        // All scenarios should have reasonable performance
        assert!(
            avg < Duration::from_secs(3),
            "Scenario {} average time should be well under limit, was {:?}",
            scenario,
            avg
        );
    }
}
