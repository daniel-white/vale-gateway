use opentelemetry::global::{
    BoxedTracer, meter, set_text_map_propagator, set_tracer_provider, tracer,
};
use opentelemetry::metrics::Meter;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::Sampler::AlwaysOn;
use opentelemetry_sdk::trace::TracerProviderBuilder;
use opentelemetry_stdout::SpanExporter;
use std::sync::LazyLock;
use tracing::subscriber::set_global_default;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

pub(crate) static TRACER: LazyLock<BoxedTracer> = LazyLock::new(|| tracer("vg-core"));
pub(crate) static METER: LazyLock<Meter> = LazyLock::new(|| meter("vg-core"));

pub fn init(name: &'static str) {
    let exporter = SpanExporter::default(); // Creates a default stdout exporter

    let tracer_provider = TracerProviderBuilder::default()
        //.with_simple_exporter(exporter)
        .with_sampler(AlwaysOn)
        .build();

    let tracer = tracer_provider.tracer(name);
    let registry = Registry::default().with(OpenTelemetryLayer::default());

    set_tracer_provider(tracer_provider);
    set_global_default(registry).expect("unable to initialize tracing");
    set_text_map_propagator(TraceContextPropagator::new());
}
