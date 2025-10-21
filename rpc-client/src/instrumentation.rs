use opentelemetry::KeyValue;
use opentelemetry::global::{BoxedTracer, meter, tracer};
use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter};
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context as TaskContext, Poll};
use std::time::Instant;
use tower::{Layer, Service};
use tracing::{Span, error, info, instrument};

pub(crate) static TRACER: LazyLock<BoxedTracer> = LazyLock::new(|| tracer("vg-rpc-client"));
pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-rpc-client"));

/// Metrics for monitoring RPC client operations
#[derive(Debug)]
pub struct ClientMetrics {
    /// Total number of RPC requests
    pub requests_total: Counter<u64>,
    /// Duration of RPC requests in seconds
    pub requests_duration: Histogram<f64>,
    /// Circuit breaker state (0=closed, 1=open, 2=half-open)
    pub circuit_breaker_state: Gauge<i64>,
    /// Total number of retry attempts
    pub retry_attempts_total: Counter<u64>,
    /// Total number of reconnection attempts
    pub reconnection_attempts_total: Counter<u64>,
    /// Number of active connections
    pub active_connections: Gauge<i64>,
}

impl ClientMetrics {
    /// Create a new ClientMetrics instance with the provided meter
    pub fn new(meter: &Meter) -> Self {
        Self {
            requests_total: meter
                .u64_counter("rpc_client_requests_total")
                .with_description("Total number of RPC requests")
                .build(),
            requests_duration: meter
                .f64_histogram("rpc_client_request_duration_seconds")
                .with_description("Duration of RPC requests in seconds")
                .build(),
            circuit_breaker_state: meter
                .i64_gauge("rpc_client_circuit_breaker_state")
                .with_description("Circuit breaker state (0=closed, 1=open, 2=half-open)")
                .build(),
            retry_attempts_total: meter
                .u64_counter("rpc_client_retry_attempts_total")
                .with_description("Total number of retry attempts")
                .build(),
            reconnection_attempts_total: meter
                .u64_counter("rpc_client_reconnection_attempts_total")
                .with_description("Total number of reconnection attempts")
                .build(),
            active_connections: meter
                .i64_gauge("rpc_client_active_connections")
                .with_description("Number of active connections")
                .build(),
        }
    }
}

impl Default for ClientMetrics {
    fn default() -> Self {
        Self::new(&METER)
    }
}

/// Tower layer for adding OpenTelemetry instrumentation to RPC client operations
#[derive(Debug, Clone)]
pub struct InstrumentationLayer {
    metrics: Arc<ClientMetrics>,
}

impl InstrumentationLayer {
    /// Create a new instrumentation layer with the provided metrics
    pub fn new(metrics: Arc<ClientMetrics>) -> Self {
        Self { metrics }
    }
}

impl Default for InstrumentationLayer {
    fn default() -> Self {
        let metrics = Arc::new(ClientMetrics::default());
        Self { metrics }
    }
}

impl<S> Layer<S> for InstrumentationLayer {
    type Service = InstrumentationService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InstrumentationService {
            inner,
            metrics: self.metrics.clone(),
        }
    }
}

/// Service wrapper that adds OpenTelemetry instrumentation
#[derive(Debug, Clone)]
pub struct InstrumentationService<S> {
    inner: S,
    metrics: Arc<ClientMetrics>,
}

impl<S, Request> Service<Request> for InstrumentationService<S>
where
    S: Service<Request>,
    S::Error: std::fmt::Display,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = InstrumentationFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[instrument(skip(self, request), fields(rpc.system = "jsonrpc", rpc.service = "configuration"))]
    fn call(&mut self, request: Request) -> Self::Future {
        let start_time = Instant::now();

        // Increment request counter
        self.metrics
            .requests_total
            .add(1, &[KeyValue::new("method", "rpc_request")]);

        info!("Starting RPC client request");

        InstrumentationFuture {
            future: self.inner.call(request),
            metrics: self.metrics.clone(),
            start_time,
        }
    }
}

/// Future wrapper that handles span completion and metrics recording
#[pin_project]
pub struct InstrumentationFuture<F> {
    #[pin]
    future: F,
    metrics: Arc<ClientMetrics>,
    start_time: Instant,
}

impl<F, T, E> Future for InstrumentationFuture<F>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.future.poll(cx) {
            Poll::Ready(result) => {
                let duration = this.start_time.elapsed();

                // Record request duration
                this.metrics.requests_duration.record(
                    duration.as_secs_f64(),
                    &[KeyValue::new("method", "rpc_request")],
                );

                // Log result and add span attributes
                match &result {
                    Ok(_) => {
                        info!(
                            duration_ms = duration.as_millis(),
                            status = "success",
                            "RPC client request completed successfully"
                        );
                        Span::current().record("rpc.response.status", "success");
                    }
                    Err(error) => {
                        error!(
                            duration_ms = duration.as_millis(),
                            status = "error",
                            error = %error,
                            "RPC client request failed"
                        );
                        Span::current().record("rpc.response.status", "error");
                        Span::current().record("rpc.response.error", error.to_string());
                    }
                }

                Span::current().record("rpc.request.duration_ms", duration.as_millis() as i64);

                Poll::Ready(result)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Helper trait for adding instrumentation to services
pub trait InstrumentedService<S> {
    /// Add instrumentation layer to the service
    fn with_instrumentation(self, layer: InstrumentationLayer) -> InstrumentationService<S>;
}

impl<S> InstrumentedService<S> for S {
    fn with_instrumentation(self, layer: InstrumentationLayer) -> InstrumentationService<S> {
        layer.layer(self)
    }
}
